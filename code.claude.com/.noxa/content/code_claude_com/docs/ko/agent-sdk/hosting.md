# Agent SDK 호스팅
## ​호스팅 요구사항
## ​SDK 아키텍처 이해
## ​샌드박스 제공자 옵션
## ​프로덕션 배포 패턴
## ​FAQ
## ​다음 단계







프로덕션 환경에서 Claude Agent SDK 배포 및 호스팅

Claude Agent SDK는 기존의 상태 비저장 LLM API와 달리 대화 상태를 유지하고 지속적인 환경에서 명령을 실행합니다. 이 가이드는 프로덕션에서 SDK 기반 에이전트를 배포하기 위한 아키텍처, 호스팅 고려사항 및 모범 사례를 다룹니다.
기본 샌드박싱을 넘어선 보안 강화(네트워크 제어, 자격증명 관리 및 격리 옵션 포함)에 대해서는 [보안 배포](https://code.claude.com/docs/ko/agent-sdk/secure-deployment)를 참조하십시오.


## [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%ED%98%B8%EC%8A%A4%ED%8C%85-%EC%9A%94%EA%B5%AC%EC%82%AC%ED%95%AD) 호스팅 요구사항


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%BB%A8%ED%85%8C%EC%9D%B4%EB%84%88-%EA%B8%B0%EB%B0%98-%EC%83%8C%EB%93%9C%EB%B0%95%EC%8B%B1) 컨테이너 기반 샌드박싱


보안 및 격리를 위해 SDK는 샌드박싱된 컨테이너 환경 내에서 실행되어야 합니다. 이는 프로세스 격리, 리소스 제한, 네트워크 제어 및 임시 파일 시스템을 제공합니다.
SDK는 또한 명령 실행을 위한 [프로그래밍 방식의 샌드박스 구성](https://code.claude.com/docs/ko/agent-sdk/typescript#sandbox-settings)을 지원합니다.


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%8B%9C%EC%8A%A4%ED%85%9C-%EC%9A%94%EA%B5%AC%EC%82%AC%ED%95%AD) 시스템 요구사항


각 SDK 인스턴스에는 다음이 필요합니다:


- **런타임 종속성**
  - Python SDK의 경우 Python 3.10+ 또는 TypeScript SDK의 경우 Node.js 18+
  - 두 SDK 패키지 모두 호스트 플랫폼용 네이티브 Claude Code 바이너리를 번들로 포함하므로 생성된 CLI에 대해 별도의 Claude Code 또는 Node.js 설치가 필요하지 않습니다.
- **리소스 할당**
  - 권장: 1GiB RAM, 5GiB 디스크 및 1 CPU(필요에 따라 작업에 맞게 조정)
- **네트워크 액세스**
  - `api.anthropic.com`으로의 아웃바운드 HTTPS
  - 선택사항: MCP 서버 또는 외부 도구에 대한 액세스


## [​](https://code.claude.com/docs/ko/agent-sdk/hosting#sdk-%EC%95%84%ED%82%A4%ED%85%8D%EC%B2%98-%EC%9D%B4%ED%95%B4) SDK 아키텍처 이해


상태 비저장 API 호출과 달리 Claude Agent SDK는 다음을 수행하는 **장기 실행 프로세스**로 작동합니다:


- **명령 실행** - 지속적인 셸 환경에서
- **파일 작업 관리** - 작업 디렉토리 내에서
- **도구 실행 처리** - 이전 상호작용의 컨텍스트 포함


## [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%83%8C%EB%93%9C%EB%B0%95%EC%8A%A4-%EC%A0%9C%EA%B3%B5%EC%9E%90-%EC%98%B5%EC%85%98) 샌드박스 제공자 옵션


여러 제공자가 AI 코드 실행을 위한 보안 컨테이너 환경을 전문으로 합니다:


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [데모 구현](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


자체 호스팅 옵션(Docker, gVisor, Firecracker) 및 상세한 격리 구성에 대해서는 [격리 기술](https://code.claude.com/docs/ko/agent-sdk/secure-deployment#isolation-technologies)을 참조하십시오.


## [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%ED%94%84%EB%A1%9C%EB%8D%95%EC%85%98-%EB%B0%B0%ED%8F%AC-%ED%8C%A8%ED%84%B4) 프로덕션 배포 패턴


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%ED%8C%A8%ED%84%B4-1-%EC%9E%84%EC%8B%9C-%EC%84%B8%EC%85%98) 패턴 1: 임시 세션


각 사용자 작업에 대해 새 컨테이너를 생성한 후 완료되면 삭제합니다.
일회성 작업에 최적이며, 사용자는 작업이 완료되는 동안 AI와 상호작용할 수 있지만 완료되면 컨테이너가 삭제됩니다.
**예시:**


- 버그 조사 및 수정: 관련 컨텍스트를 사용하여 특정 문제를 디버깅하고 해결
- 송장 처리: 회계 시스템을 위해 영수증/송장에서 데이터 추출 및 구조화
- 번역 작업: 언어 간 문서 또는 콘텐츠 배치 번역
- 이미지/비디오 처리: 미디어 파일에서 변환, 최적화 또는 메타데이터 추출 적용


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%ED%8C%A8%ED%84%B4-2-%EC%9E%A5%EA%B8%B0-%EC%8B%A4%ED%96%89-%EC%84%B8%EC%85%98) 패턴 2: 장기 실행 세션


장기 실행 작업을 위해 지속적인 컨테이너 인스턴스를 유지합니다. 종종 수요에 따라 컨테이너 내에서 **여러** Claude Agent 프로세스를 실행합니다.
사용자 입력 없이 조치를 취하는 사전 예방적 에이전트, 콘텐츠를 제공하는 에이전트 또는 많은 양의 메시지를 처리하는 에이전트에 최적입니다.
**예시:**


- 이메일 에이전트: 수신 이메일을 모니터링하고 콘텐츠에 따라 자동으로 분류, 응답 또는 조치 수행
- 사이트 빌더: 컨테이너 포트를 통해 제공되는 라이브 편집 기능이 있는 사용자별 맞춤 웹사이트 호스팅
- 고빈도 채팅봇: Slack과 같은 플랫폼에서 빠른 응답 시간이 중요한 지속적인 메시지 스트림 처리


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%ED%8C%A8%ED%84%B4-3-%ED%95%98%EC%9D%B4%EB%B8%8C%EB%A6%AC%EB%93%9C-%EC%84%B8%EC%85%98) 패턴 3: 하이브리드 세션


데이터베이스에서 또는 SDK의 세션 재개 기능에서 가져온 기록 및 상태로 수화된 임시 컨테이너입니다.
사용자의 간헐적인 상호작용으로 작업을 시작하고 작업이 완료되면 종료되지만 계속할 수 있는 컨테이너에 최적입니다.
**예시:**


- 개인 프로젝트 관리자: 간헐적인 체크인으로 진행 중인 프로젝트 관리를 지원하고 작업, 결정 및 진행 상황의 컨텍스트 유지
- 심층 연구: 수 시간의 연구 작업을 수행하고 결과를 저장하며 사용자가 돌아올 때 조사 재개
- 고객 지원 에이전트: 여러 상호작용에 걸친 지원 티켓을 처리하고 티켓 기록 및 고객 컨텍스트 로드


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%ED%8C%A8%ED%84%B4-4-%EB%8B%A8%EC%9D%BC-%EC%BB%A8%ED%85%8C%EC%9D%B4%EB%84%88) 패턴 4: 단일 컨테이너


하나의 글로벌 컨테이너에서 여러 Claude Agent SDK 프로세스를 실행합니다.
밀접하게 협력해야 하는 에이전트에 최적입니다. 에이전트가 서로를 덮어쓰지 않도록 방지해야 하므로 이것이 가장 인기 없는 패턴일 가능성이 높습니다.
**예시:**


- **시뮬레이션**: 비디오 게임과 같은 시뮬레이션에서 서로 상호작용하는 에이전트입니다.


## [​](https://code.claude.com/docs/ko/agent-sdk/hosting#faq) FAQ


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%83%8C%EB%93%9C%EB%B0%95%EC%8A%A4%EC%99%80-%EC%96%B4%EB%96%BB%EA%B2%8C-%ED%86%B5%EC%8B%A0%ED%95%A9%EB%8B%88%EA%B9%8C) 샌드박스와 어떻게 통신합니까?


컨테이너에서 호스팅할 때 SDK 인스턴스와 통신하기 위해 포트를 노출합니다. 애플리케이션은 외부 클라이언트를 위해 HTTP/WebSocket 엔드포인트를 노출할 수 있으며 SDK는 컨테이너 내에서 내부적으로 실행됩니다.


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%BB%A8%ED%85%8C%EC%9D%B4%EB%84%88-%ED%98%B8%EC%8A%A4%ED%8C%85-%EB%B9%84%EC%9A%A9%EC%9D%80-%EC%96%BC%EB%A7%88%EC%9E%85%EB%8B%88%EA%B9%8C) 컨테이너 호스팅 비용은 얼마입니까?


에이전트 제공의 주요 비용은 토큰입니다. 컨테이너는 프로비저닝하는 항목에 따라 다르지만 최소 비용은 대략 시간당 5센트입니다.


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%9C%A0%ED%9C%B4-%EC%BB%A8%ED%85%8C%EC%9D%B4%EB%84%88%EB%A5%BC-%EC%A2%85%EB%A3%8C%ED%95%B4%EC%95%BC-%ED%95%A0-%EB%95%8C%EC%99%80-%EB%94%B0%EB%9C%BB%ED%95%98%EA%B2%8C-%EC%9C%A0%EC%A7%80%ED%95%B4%EC%95%BC-%ED%95%A0-%EB%95%8C%EB%8A%94-%EC%96%B8%EC%A0%9C%EC%9E%85%EB%8B%88%EA%B9%8C) 유휴 컨테이너를 종료해야 할 때와 따뜻하게 유지해야 할 때는 언제입니까?


이는 제공자에 따라 다를 가능성이 높으며, 다양한 샌드박스 제공자는 샌드박스가 종료될 수 있는 유휴 타임아웃에 대해 다양한 기준을 설정할 수 있습니다.
사용자 응답이 얼마나 자주 발생할 것으로 예상되는지에 따라 이 타임아웃을 조정하고 싶을 것입니다.


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#claude-code-cli%EB%A5%BC-%EC%96%BC%EB%A7%88%EB%82%98-%EC%9E%90%EC%A3%BC-%EC%97%85%EB%8D%B0%EC%9D%B4%ED%8A%B8%ED%95%B4%EC%95%BC-%ED%95%A9%EB%8B%88%EA%B9%8C) Claude Code CLI를 얼마나 자주 업데이트해야 합니까?


Claude Code CLI는 semver로 버전이 지정되므로 모든 주요 변경사항이 버전이 지정됩니다.


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%BB%A8%ED%85%8C%EC%9D%B4%EB%84%88-%EC%83%81%ED%83%9C-%EB%B0%8F-%EC%97%90%EC%9D%B4%EC%A0%84%ED%8A%B8-%EC%84%B1%EB%8A%A5%EC%9D%84-%EC%96%B4%EB%96%BB%EA%B2%8C-%EB%AA%A8%EB%8B%88%ED%84%B0%EB%A7%81%ED%95%A9%EB%8B%88%EA%B9%8C) 컨테이너 상태 및 에이전트 성능을 어떻게 모니터링합니까?


컨테이너는 단지 서버이므로 백엔드에 사용하는 동일한 로깅 인프라가 컨테이너에서 작동합니다.


### [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EC%97%90%EC%9D%B4%EC%A0%84%ED%8A%B8-%EC%84%B8%EC%85%98%EC%9D%B4-%ED%83%80%EC%9E%84%EC%95%84%EC%9B%83%EB%90%98%EA%B8%B0-%EC%A0%84%EC%97%90-%EC%96%BC%EB%A7%88%EB%82%98-%EC%98%A4%EB%9E%98-%EC%8B%A4%ED%96%89%EB%90%A0-%EC%88%98-%EC%9E%88%EC%8A%B5%EB%8B%88%EA%B9%8C) 에이전트 세션이 타임아웃되기 전에 얼마나 오래 실행될 수 있습니까?


에이전트 세션은 타임아웃되지 않지만 Claude가 루프에 갇히는 것을 방지하기 위해 ‘maxTurns’ 속성을 설정하는 것을 고려하십시오.


## [​](https://code.claude.com/docs/ko/agent-sdk/hosting#%EB%8B%A4%EC%9D%8C-%EB%8B%A8%EA%B3%84) 다음 단계


- [보안 배포](https://code.claude.com/docs/ko/agent-sdk/secure-deployment) - 네트워크 제어, 자격증명 관리 및 격리 강화
- [TypeScript SDK - 샌드박스 설정](https://code.claude.com/docs/ko/agent-sdk/typescript#sandbox-settings) - 프로그래밍 방식으로 샌드박스 구성
- [세션 가이드](https://code.claude.com/docs/ko/agent-sdk/sessions) - 세션 관리에 대해 알아보기
- [권한](https://code.claude.com/docs/ko/agent-sdk/permissions) - 도구 권한 구성
- [비용 추적](https://code.claude.com/docs/ko/agent-sdk/cost-tracking) - API 사용량 모니터링
- [MCP 통합](https://code.claude.com/docs/ko/agent-sdk/mcp) - 맞춤 도구로 확장[Claude Code Docs home page](https://code.claude.com/docs/ko/overview)

[Privacy choices](https://code.claude.com/docs/ko/agent-sdk/hosting#)

