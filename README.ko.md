# OpenWrt MCP

Rust로 개발하는 OpenWrt 관리용 MCP 서버입니다. 에이전트가 OpenWrt를 제어하되, 사용자가 기능별 접근 권한과 실행 권한을 정할 수 있도록 합니다. OpenWrt 공식 프로젝트와 무관한 커뮤니티 프로젝트입니다.

## 핵심 목표

1. OpenWrt 기본 기능과 설치 패키지의 기능까지 관리할 수 있는 확장 구조.
2. 입력 검증, 구조화된 응답, 실제 상태 확인을 통한 정확성과 속도.
3. 단일 Rust 프로세스와 사용량 제한을 통한 낮은 리소스 소비.
4. 카테고리별 `차단 / 읽기 / 읽기·쓰기`와 별도 `실행` 설정.
5. 기능을 추가해도 유지해야 하는 문서화된 계층과 자동 아키텍처 검사.
6. 호출 시도·거절·실행 결과의 감사 기록과 일반적인 로그 설정.
7. 암호화뿐 아니라 공통 기능 전체를 Windows·Linux·macOS에서 사용할 수 있는 구조.

코드는 **core / features / runtime / adapters / mcp / server** 여섯 계층과 개발용 **xtask**로 나뉩니다. 권한·검증은 core, 범주별 작업 정의는 features, 실행 흐름은 runtime, 실제 장치·로그 접근은 adapters, 통신은 mcp, 조립·설정은 server가 담당합니다. 프로토콜은 공통 실행기를 통해서만 장치에 접근합니다.

아키텍처 하네스는 의존성·소스 경계·디렉터리 소유권을 검사합니다. 새 요구가 경계를 바꿔야 한다면 **요구사항 → 설계 결정(ADR) → 아키텍처·하네스 확장 → 구조 검증 → 기능 구현** 순서를 지켜야 합니다. 자세한 규칙은 [개발 절차](docs/development.md)와 [기계 판독 계약](architecture/spec.toml)에 있습니다.

## 현재 구현

v0.1 개발 단계입니다. 현재 시스템·네트워크·무선·DHCP/DNS·서비스·스토리지·진단의 조회 25개와 APK 패키지 조회 1개, 총 26개를 구현했습니다. 기존 조회 중 16개는 형식과 크기를 명시한 목록·레코드 응답을 사용하며, 패키지 조회는 전체 응답 검증 후 16개씩 페이지로 제공합니다. MCP 표준입출력, 기본 차단 권한 엔진, 감사 로그, 리소스 제한과 자동 테스트를 제공합니다. [스토리지 조회](docs/storage-observations.md)는 LuCI가 반환한 경로·파일시스템·용량에 한정하며, 전체 디스크 목록이나 파일 내용·설정 변경을 제공하지 않습니다.

[DHCPv4·DHCPv6 조회](docs/dhcp-observations.md)는 `dhcp_dns.read` 권한으로 LuCI가 보고한 임대 정보를 읽습니다. IP·MAC·DUID·호스트명이 공개되므로 필요한 경우 개별 도구를 차단하세요. 중복 행과 만료 시간의 `false` 값을 그대로 보존하며, 전체 임대 목록·DNS 상태·현재 접속 여부를 보증하지 않습니다. v10 설계·하네스 체크포인트를 검증한 뒤 구현했습니다. 스토리지 응답 계약도 v2로 강화하여 정상 데이터와 오류가 함께 온 응답을 거부합니다.

[인터페이스 IP 조회](docs/interface-ip-observations.md)는 지정한 인터페이스의 주소·경로·netifd 관리 이웃 항목을 `network.read`, DNS 서버·검색 도메인을 `dhcp_dns.read`로 제공합니다. 누락된 목록을 빈 목록으로 바꾸지 않으며, 실제 DNS 질의나 이웃 탐색을 실행하지 않습니다. 커널 전체 경로·ARP/NDP 캐시·현재 연결 상태와는 구분해야 합니다. 주소와 식별자 공개를 원하지 않으면 해당 도구를 개별 차단하세요.

[v10 에뮬레이터 검증](docs/emulator-validation-v10.md)에서는 실제 MCP·SSH로 인터페이스 조회 4종, 마운트 3개, 빈 블록 장치·DHCP 임대 응답을 검증했습니다. 데이터가 없는 경우의 성공을 실제 디스크·임대·IPv6 환경까지 검증한 것으로 표현하지 않습니다.

Linux-on-WSL과 [Windows GNU 네이티브 전체 검사](docs/windows-validation.md)를 실행하고 있으며, 최신 개수와 범위는 [검증 기록](docs/validation.md)에 구분해 남깁니다. 별도의 [패키지 에뮬레이터 검증](docs/emulator-validation-v7.md)에서는 APK가 보고한 205개를 13페이지로 조회하고 재호출 일관성, 새 조회 후 이전 토큰 차단, 안전한 감사 로그를 확인했습니다. 앞선 [v6 에뮬레이터 검증](docs/emulator-validation-v6.md)의 성공 조회 12개와 오류 처리 2개는 별도 기록입니다. 무선 단말·국가 조회는 아직 모의 데이터 기반 검증입니다. Windows 보호 파일 읽기를 추가했으며, BPI-R4 하드웨어, MSVC·macOS 검증과 macOS 보호 파일·Vault 연동은 남아 있습니다.

`read_write`만 허용해도 재시작·설정 적용 권한이 자동으로 생기지 않습니다. 작업마다 필요한 카테고리와 권한을 모두 검사하며, 목록에서 숨겨진 도구를 직접 호출해도 같은 검사를 거칩니다.

전체 OpenWrt 관리 기능은 아직 구현 완료 상태가 아닙니다. UCI 변경, 암호화 백업과 복구, opkg 및 패키지 설치·삭제, 펌웨어 관리, 보호 인터페이스 설정, 실장비 검증은 후속 단계입니다. 범용 프로그램 실행만으로 전체 기능을 검증했다고 보지 않습니다.

## 문서와 검증

아키텍처 v3에서는 키 보관 위치(`key-sources`), ZIP 내부 항목 선택, 암호화(`crypto-age`)를 분리했습니다. 공개키로 암호화하고 개인키는 복호화에만 사용하는 내부 기반 기능을 제공합니다. 환경변수·Linux 및 Windows 제한 파일·일반 ZIP을 지원하며, Vault의 실제 권한 검증과 암호화 백업 MCP 작업은 아직 미구현입니다. [키 관리와 지원 범위](docs/key-management.md)를 확인하세요.

v4에서는 MCP가 실행되는 컴퓨터와 관리 대상 OpenWrt를 분리했습니다. 기본값은 대상 미설정이며, 내장 SSH 연결 또는 검증된 OpenWrt 장치의 로컬 실행을 명시적으로 선택합니다. SSH 실패 시 PC에서 대신 실행하지 않습니다. OS별 설정·키·감사 파일 보호는 `host-platform`, 공통 원격 실행은 `backend-ssh`가 담당합니다. 환경변수 설정·키 소스와 stderr 로그가 공통 경로입니다. v8에서 Windows의 로컬 NTFS 보호 파일 읽기를 추가했습니다. macOS 보호 파일, Windows 파일 로그와 Vault 연동은 명시적으로 미지원입니다.

Windows는 절대 경로의 설정·키 파일과 제한된 ZIP 내부 항목을 읽을 수 있습니다. 실제 소유자·접근 권한, 상위 폴더, 열린 파일 핸들, 드라이브와 링크를 검사하며 단순한 ‘읽기 전용’ 속성을 보안 증거로 사용하지 않습니다. 다른 계정이 수정할 수 있는 상위 경로, 경로 연결 우회, 하드링크, 지원하지 않는 저장 장치는 거부합니다. [설정 방법과 제한](docs/windows-protected-files.md)을 참고하세요. 일반 보호 파일 지원이 OneDrive Vault의 잠금·인증 연동까지 구현됐다는 뜻은 아닙니다.

세 OS의 네이티브 CI와 필수 테스트를 하네스로 강제합니다. 현재 실제 실행 증거는 Linux-on-WSL과 별도로 실행한 Windows GNU 네이티브 전체 검사이며, 원격 CI 실행이나 MSVC·macOS 검증을 의미하지 않습니다. [플랫폼별 구현·검증 범위](docs/platform-support.md)를 확인하세요.

v5에서는 실제 대상의 API 입력 서명을 확인한 뒤 실행을 허용합니다. 버전 문자열만으로 지원을 가정하지 않고, 권한 때문에 보이지 않거나 서명이 불완전한 API는 미지원으로 단정하지 않습니다. `operation_capability`로 상태를 확인하며, 미확인·불일치 작업은 실행하지 않습니다. 같은 연결의 관측을 최대 30초 보관하고, 새 탐지가 실패하면 이전 성공으로 되돌아가지 않습니다. 설계 체크포인트 이후 구현과 행동 테스트를 추가했습니다. [호환성 검사와 한계](docs/capabilities.md), [기준 장치 기록](docs/reference-target.md), [전체 구현 계획](docs/implementation-plan.md)을 확인하세요.

v6에서는 별도의 설계·하네스 체크포인트 이후 인터페이스 목록, 무선 장치 목록, 단일 서비스 상태, 서비스 목록을 추가했습니다. `network_interface_status`는 항상 고정된 `network.interface.dump {}` 결과에서 정확히 일치하는 인터페이스를 선택합니다. 기존 `status` 입력 서명을 우회하거나 실패 후 다른 동작으로 재시도하지 않습니다. 응답 계약 v2는 `/up` 같은 이전 키 대신 `interface`, `up` 등의 일반 필드명을 사용하므로 이전 개발 버전과 결과 형식이 다릅니다.

형식이 맞지 않는 필드, 중복 식별자, 과도한 목록은 부분 결과로 반환하지 않습니다. 응답에는 검토된 필드만 포함하며, 중첩 목록은 합산 256개, 정규화 결과는 64KiB, MCP 도구 결과 전체는 256KiB로 제한합니다. 공통 JSON 해석기는 중복 키·과도한 깊이도 거부합니다. 구체적인 범위는 [목록 조회 계약](docs/collection-read-contracts.md)과 [아키텍처 v6](docs/adr/0006-bounded-read-projections.md)에 있습니다.

- [요구사항과 성능 목표](docs/requirements.md)
- [아키텍처와 모듈 계약](docs/architecture.md)
- [아키텍처 우선 개발 절차](docs/development.md)
- [기능별 구현 범위](docs/coverage.md)
- [보안 모델과 현재 한계](docs/security.md)
- [권한·로그 설정](docs/configuration.md)
- [빌드 및 검증 결과](docs/validation.md)
- [영문 사용 안내](README.md)

시스템·네트워크만 읽는 예제는 [read-only.toml](config/read-only.toml), 현재 구현된 읽기 범주를 모두 허용하는 예제는 [observability.toml](config/observability.toml)입니다. 두 예제 모두 실행 권한이 없고, 빠진 카테고리는 차단됩니다. 실제 장치에 연결하지 않고 `check`와 `catalog`로 정책과 목록을 확인할 수 있습니다. `tools/Test-Repository.ps1`은 아키텍처·진화 검사, 권한·프로토콜·로그 테스트, 릴리스 빌드를 수행합니다.

일반 Services.Read 권한은 VPN·방화벽 관련 데몬을 포함한 서비스 이름과 실행/PID/종료 코드 메타데이터도 공개하지만, 명령행·환경변수·설정은 공개하지 않고 재시작 권한도 부여하지 않습니다. logd/sysntpd의 고정 조회만 허용하려면 작업 차단 목록에서 `service_status`, `service_status_list`를 차단하세요.

무선 읽기 권한은 연결 단말 MAC 주소와 링크 통계도 공개합니다. 단말 식별자를 숨기려면 `wireless_stations`, `wireless_station_status`를 차단하세요. 이 조회 3개는 [v6 구조 안에서 구현](docs/wireless-observation-contracts.md)했으며, 스캔·연결 해제·국가 설정은 하지 않습니다. 빈 목록은 드라이버 실패와 구분되지 않을 수 있으므로 단말 부재나 정상 상태의 증거가 아닙니다.

`packages_apk_installed`는 `packages.read` 권한으로 APK 3.0.5의 설치 패키지 응답을 조회합니다. `{}`로 새 목록을 만들고 `next_cursor`를 그대로 `cursor`에 넣어 다음 페이지를 요청하세요. 목록은 조회 시작 후 120초 동안 유효하며, 새 조회는 이전 토큰을 무효화합니다. 이름·버전·아키텍처와 계층을 보존하지만 다른 패키지 관리자나 읽히지 않은 계층까지 포함한 장치 전체 상태라는 뜻은 아닙니다. 모든 응답에 이 범위를 명시합니다. [사용법과 제한](docs/package-observations.md), [설계·하네스 v7](docs/adr/0007-paged-package-observations.md)을 참고하세요.
