# 전체 OpenWrt 관리 기능 명세

문서 버전: 2. 갱신: 2026-09-14 (Asia/Seoul). 최초 기준 `a711593`, P1/v20 조회 증분 반영.
상태: **개발 목표 명세**. 기능 구현 완료, 신규 아키텍처 승인, 라우터 변경 또는 배포 승인이 아니다.
현재 허용 경계는 [architecture/spec.toml](../architecture/spec.toml)의 v20이다.
아래 미래 기능이 그 경계를 넘으면 ADR·명세·하네스를 먼저 확장하고 검증·커밋한다.

관련 문서: [단계별 구현 계획](implementation-plan.md), [현재 구현 범위](coverage.md),
[변경 작업 설계안](management-workflows.md), [기존 개발 이력](implementation-history.md).

## 1. 전체 지원의 판정 단위

목표는 단순한 명령 실행이 아니라 **관찰 → 계획 → 변경/실행 → 실제 상태 검증 → 확인/복구**의 관리 흐름이다.
아래 ID는 기능 단위이며 MCP 도구 수가 아니다. 하나의 기능을 여러 도구로 나눌 수 있다.

지원 프로필은 장치 모델·보드/저장장치·OpenWrt 빌드·설치 패키지/수정본·드라이버·API 형식으로 식별한다.
최초 기준은 [기록된 BPI-R4 설치](reference-target.md)의 OpenWrt 25.12.5 / mediatek/filogic이다.
이는 2026-09-13에 관찰한 이력이지 현재 라우터 상태가 아니다. 수락 검사 전에 다시 확인해야 한다.

전체성은 다음 목록을 서로 대조한 **프로필별 관리 표면 대장**으로 판정한다.

1. 기본 시스템과 실제 설치 패키지의 설정 스키마·설정 파일·서비스·공식 관리 API/CLI.
2. 허용된 범위에서 관찰한 ubus 객체/메서드, UCI 섹션/옵션 종류, 서비스·드라이버 기능.
3. CLI 전용·비 UCI 설정, 일회성 실행, 부팅/복구, LuCI에만 노출된 관리 동작.
4. 각 표면에 연결된 기능 ID, 구현 계약/버전, 필요한 권한, 테스트 증거, 남은 차이.

새 옵션·패키지·변형은 자동 실행 도구로 만들지 않는다. 미지원 항목을 추가하고 검토된 어댑터로 확장한다.
미설치/하드웨어 부재는 근거가 있을 때만 `해당 없음`이다. ACL에 가려졌거나 관찰에 실패한 상태는 `미확인`이다.
필수 항목의 미지원·미확인·일부 지원이 남으면 그 프로필의 전체 지원을 선언하지 않는다.
안전 조건 때문에 의도적으로 차단하는 동작도 이유와 대체 절차를 기록한다. 미구현 동작의 차단을 구현 완료로 계산하지 않는다.
이미 구현·검증된 동작을 사용자의 권한 정책이 차단하는 것은 별도의 인가 상태이며 구현 누락이 아니다.
미래의 모든 외부 패키지를 선제적으로 구현했다는 보편적 완성 주장은 하지 않는다.

### 기준선

- 현재 기본 관리 조회 55개: 형식화된 응답 44개, 기존 스칼라 응답 9개, 패키지 조회 2개. 이 중 UCI 조회 28개.
- `operation_capability`는 별도의 지원 상태 메타데이터 도구이며 관리 기능 55개에 포함하지 않는다.
- 기본 변경/실행 관리 도구는 아직 없다. 암호화·아카이브·영향 그래프는 내부 기반이지 실제 백업/복구 기능이 아니다.
- v20의 LED·Dropbear·uHTTPd·odhcpd 조회 4종을 구현했다. [정확한 필드 계약](system-service-uci-observations.md)과 [P1 대장](management-inventory.md)을 따른다. 해당 기능군의 변경/실행 완료가 아니다.
- 호스트 테스트, 에뮬레이터 사용자 공간, 실기기 증거는 별도로 유지한다. 테스트 개수로 기능 완성률을 계산하지 않는다.

## 2. 모든 기능에 적용할 계약

### 기능 세부 계약의 필수 필드

구현 착수 때 아래 내용을 기능 ID 아래의 버전별 계약으로 확정한다. 표의 범위만으로 임의 옵션을 허용하면 안 된다.

| 필드 | 반드시 명세할 내용 |
| --- | --- |
| ID / 프로필 / 버전 | 이 문서의 ID, 대상 구성요소/드라이버, 정확한 입력·출력 계약 버전, 아키텍처 체크포인트 |
| 입력 | 타입·단위·길이·범위·열거값·필수/생략 의미·목록 순서/중복, 대상 식별자, 허용된 비밀 참조 |
| 출력 | 선택한 필드와 민감도, 구성/실행 상태의 구분, 관찰 시점·범위·완전성, 생략/빈 값/오류 의미 |
| 권한·영향 | 기능 주 카테고리와 의존 카테고리 전체, 읽기/쓰기/실행 조건, 공유 자원/전역 영향 |
| 전제조건 | 같은 대상의 신선한 기능 정보, 기준 상태/부팅 세대, 저장공간, 충돌·보호 자원·키 가용성 |
| 실패 | 안전한 고정 오류 코드, 미제출/실패/부분 적용/결과 불명 구분, 취소 가능 구간과 재조회 방법 |
| 검증·복구 | 구성과 실제 동작의 사후조건, 복구 보장 범위·기한·필요 권한, 복구 실패 처리 |
| 자원 | 입력/출력/보관량/항목 수/동시성/작업 시간 상한, 페이지·캐시 수명, 큰 작업의 별도 예산 |
| 증거 | 독립 정상/거부/장애 테스트, 호스트별 실행, 대상별 수락 결과와 정확한 소스/아티팩트 |

현재 64 KiB 정규화 결과·256 KiB MCP 결과·256개 공유 항목 상한 등을 그대로 존중한다.
더 큰 인벤토리는 검토된 불변 페이지 스냅샷 계약을 추가한다. 무제한 응답이나 조용한 잘라내기는 금지한다.
설정 값에 대한 런타임 기본값을 임의로 합성하지 않는다. 변경 검증용 재조회는 계획 당시 결과/캐시를 재사용하지 않는다.

### 권한 의미

- `R`: 부작용을 검토한 읽기. 실행 파일을 내부에서 호출한다는 이유만으로 사용자 Execute가 필요한 것은 아니다.
- `W`: 설정 생성/수정/삭제/순서 변경. 해당 카테고리 `read_write`가 필요하다.
- `X`: 시작/중지/재시작/갱신/진단 트래픽/설치/플래시 등 운영 동작. 해당 카테고리의 독립 `execute`가 필요하다.
- 설정의 적용·재시작을 동반하는 `W+X`는 둘 다 필요하다. Execute는 Deny를 우회하지 못한다.
- 검증·백업·복구가 접근하는 카테고리도 포함한다. 상위 카테고리 하나 또는 generic Services/Extensions로 우회하지 않는다.
- `V`는 사후 검증, `B`는 백업/복구 절차이며 **새로운 권한 비트가 아니다**. 내부 동작의 R/W/X 권한을 합산한다.
- 관리 정책·키 보관 위치·동작 정의·감사 설정은 운영자가 배포하는 설정이다. 이 MCP를 통한 자기 권한 확대는 금지한다.
- 비밀은 운영자가 등록한 목적별 참조로 입력하고 값은 반환하지 않는다. 표시 가능한 공개키/인증서 메타데이터도 개별 계약으로 제한한다.
- 원시 UCI·셸·파일·패키지 후크의 무제한 접근은 전체 지원의 대체물이 아니다. 스크립트 기능은 서명/검토된 작업 템플릿으로만 확장한다.

### 공통 수락 기준

모든 아래 행은 공통 계약과 해당 행의 추가 검증을 모두 충족해야 한다.
변경 행은 암호화된 사전 백업, 신선한 영향 분석, 동일 대상에 묶인 불변 계획, 장치 측 실행 접수/복구를 요구한다.
복구가 본질적으로 불가능한 동작은 유지보수 전용 절차·명시적 비가역성·별도 승인·대역외 복구 조건이 필요하다.
권한 없음, 알 수 없는 버전/필드, 기준 상태 변경, 잘못된 타입/크기, 비밀 누출, 시간 초과를 각 계약에서 시험한다.
장치 효과가 불명확하면 실행하지 않고, 제출 이후 통신 실패는 재실행이 아니라 작업 상태 조정으로 처리한다.

## 3. 기능 목록

표의 `현황`은 P1 증분까지의 상태다: `R일부`=명시된 일부 조회만 구현, `기반`=내부 기반/공통 기능 일부,
`설계`=아키텍처 체크포인트만 존재, `미구현`=관리 기능 없음. 어느 상태도 행 전체 완료를 의미하지 않는다.
`단계`는 [계획](implementation-plan.md)의 주 납품 단계다. 앞 단계에서 필요한 조회/내부 공통부를 먼저 만들 수 있다.

### 3.1 연결·프로필·에이전트 인터페이스 (CTL)

운영자 설정과 공통 runtime/MCP 소유. 장치 접근은 실제 대상 카테고리로 검사한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| CTL-01 | stdio MCP 초기화·도구 검색·호출·안전한 오류 | stdout 프로토콜 전용, 입력 스키마/호환 버전 명시 | 기반 | P11 |
| CTL-02 | SSH / 명시적 OpenWrt-local / 미설정 모드 | 호스트·라우터 경로 분리, 대상 없는 로컬 실행 금지 | 기반 | P11 |
| CTL-03 | 대상 신원·부팅 세대·설치 구성요소 관찰 R | 재부팅/호스트 키 변경/패키지 변경 시 증거 무효화 | R일부 | P1 |
| CTL-04 | API·CLI·UCI·서비스·하드웨어 표면 대장 R | 출처별 중복/누락 대조, 원시 설정/비밀 수집 금지 | 기반 | P1 |
| CTL-05 | 패키지·드라이버별 버전 프로필 선택 R | unknown/incompatible/absent/ACL-hidden 분리, 폴백 금지 | 기반 | P1 |
| CTL-06 | 구현·권한·가용성·검증 상태 조회 R | 현재 도구와 미래 기능을 구분, 클라이언트의 지원 주장 불신 | R일부 | P1 |
| CTL-07 | 크기 제한·불변 페이지·캐시 수명 | 모든 페이지 재인가, 대상/작업 바인딩·기한·변경 무효화 | 기반 | P1 |
| CTL-08 | 작업 상태·진행·취소 가능성·재접속 조정 R/X | 취소≠롤백, 유실 응답 후 중복 실행 없음 | 미구현 | P4 |
| CTL-09 | 명세·예시·안전한 작업 안내 제공 | 클라이언트 라벨·주석·장치 문자열은 권한/명령이 아님 | 기반 | P11 |
| CTL-10 | 검토된 새 프로필/어댑터 추가 | 설치 발견만으로 승인하지 않음, 이전 응답 계약 유지/명시적 버전 변경 | 기반 | P1 |

### 3.2 정책·키·감사·호스트 보안 (SEC)

MCP를 통한 자기 설정 변경 없이 운영자가 설정한다. OS별 기능 미지원은 명시적으로 실패해야 한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| SEC-01 | 카테고리 Deny/Read/ReadWrite + Execute, 정확한 허용/거부 | 교차 카테고리·bulk·별칭·직접 호출에서도 Deny 우선 | 기반 | P2 |
| SEC-02 | 읽기 전용/제한 관리자/유지보수 정책 예시·검사·설명 | 정책 검사는 오프라인, 사용자가 실제 유효 권한 확인 가능 | 기반 | P11 |
| SEC-03 | 목적별 SecretValue 참조: Wi-Fi·VPN·계정·서비스 자격 증명 | 인가 이후 획득, 값의 JSON/argv/로그/일반 임시 파일 유출 금지 | 미구현 | P3 |
| SEC-04 | 키 위치 추상화: 명시적 환경변수·제한 파일 | 신뢰 부모/열린 핸들 검사, 원본 환경 노출 한계 문서화 | 기반 | P3 |
| SEC-05 | 키 컨테이너 추상화: 단일 ZIP의 정확한 엔트리 | 직접 키/컨테이너 동일 포트, 경로 모호성·폭탄·암호화 ZIP 지원 여부 분리 | 기반 | P3 |
| SEC-06 | 사용자가 잠금 해제한 Personal Vault: 직접 파일/ZIP | 자동 해제·MFA 우회·일반 클라우드 파일 오인 금지; 플랫폼별 지원 근거 | 미구현 | P11 |
| SEC-07 | age 기본 공급자·암호화 공급자 교체 경계 | 공개 수신자/개인 복호화 권한 분리, 완전한 인증/종료, 임의 알고리즘 협상 금지 | 기반 | P3 |
| SEC-08 | 백업 수신자 전환·키 교체 운영 절차 | 과거 아카이브 복구 가능성 점검, 원본 키 자동 삭제/생성 없음 | 미구현 | P3 |
| SEC-09 | 감사 시도·거부·시작·성공·실패·불명·복구 연결 | 작업 ID는 비밀 아님, 인수/결과/원시 오류/비밀 해시 미기록 | 기반 | P4 |
| SEC-10 | JSON/text·수준·stderr/파일/시스템 로그·회전·보존 | 실제 감사 전달 실패 정책, 용량/시간 제한, 감사와 일반 진단 로그 구분 | 기반 | P11 |
| SEC-11 | Windows/Linux/macOS 보호 파일·로그·복원 스테이징 | ACL/링크/경합/유니코드/파일시스템 특성에 대한 네이티브 시험 | 기반 | P3 |
| SEC-12 | 최소 원격 권한·호스트 키 고정·원칙별 프로세스 분리 | 운영자 확장은 샌드박스가 아님; 비밀·생산자 출력은 비신뢰 데이터 | 기반 | P11 |

### 3.3 변경 안전장치·백업·복구 (TXN)

별도 공개 카테고리를 임의로 추가하지 않는다. 백업/복구는 firmware/storage 및 내용에 해당하는 카테고리를 검토해 합산한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| TXN-01 | 형식화된 계획·안전한 차이·만료·불변 대상/기준 바인딩 | 순서 변경/부팅/다른 장치/개인 값 해시 유출 검증 | 미구현 | P4 |
| TXN-02 | 장치 단위 실행 접수·충돌·외부 변경 감지 | 여러 MCP 호스트, LuCI/CLI pending 상태를 덮어쓰지 않음 | 미구현 | P4 |
| TXN-03 | 실제 자원 그래프와 변경 전후 영향 계산 | lan3의 부모/브리지/VLAN/방화벽/멀티캐스트 간접 영향과 unknown 거부 | 기반 | P4 |
| TXN-04 | 승인된 범위의 일관성 검사·민감 바이너리 백업 수집 | 목록/크기/생산자 성공/진짜 EOF 대조, 공유 임시 파일·후크 검토 | 미구현 | P3 |
| TXN-05 | 스트리밍 archive 검사 → age → 암호문 저장 | 캡처/암호화/저장 완료 모두 필요, 부분 파일을 완성본으로 오인하지 않음 | 기반 | P3 |
| TXN-06 | 백업 목록·보존·삭제·무결성/출처/대상 메타데이터 | 승인된 artifact ID만, 원시 경로/파일 목록 은닉, 저장 확인과 내구성 분리 | 미구현 | P3 |
| TXN-07 | 암호화된 장치 복구 권한·저널·부팅 복구 | 보관용 age 개인키를 라우터로 복사하지 않음, 재부팅/전원 손실 증거 필요 | 미구현 | P4 |
| TXN-08 | 적용·검증·확인·만료·복구 상태 기계 | confirm/timeout 경쟁의 단일 종결, 기록 없는 성공 없음 | 미구현 | P4 |
| TXN-09 | 신호 유실/호스트 종료/중복 요청/실행 결과 불명 조정 | 미제출과 제출 후 불명 구분, 자동 재적용 금지 | 미구현 | P4 |
| TXN-10 | 전체 인증 후 제한 스테이징·복원 계획·적용 | 저장소/동기화 외부, 변조/중복/링크/탈출/잘못된 대상 거부 | 기반 | P4 |
| TXN-11 | 복원 후 구성·서비스·연결·보호 자원 재검증 | 백업 존재 또는 종료 코드만으로 복구 성공 판단 금지 | 미구현 | P4 |
| TXN-12 | 비가역 유지보수·대역외 복구·명시적 승인 경계 | 파티션/패키지 후크/플래시의 가짜 롤백 금지, 보호 예외는 운영자만 설정 | 미구현 | P10 |

### 3.4 시스템·관리 접속 (SYS)

주 카테고리 system. 서비스 실행에는 검토된 services 및 실제 영향 카테고리를 추가한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| SYS-01 | 보드·릴리스·커널·가동시간·CPU/메모리·부하 R | 하드웨어 정체성과 실행 상태 분리, 선택된 비민감 정보 | R일부 | P5 |
| SYS-02 | 호스트 이름·시간대·시스템 기본 설정 R/W/V/B | 검토된 단일 설정을 첫 변경 후보로 사용, 재시작 전체 영향 검사 | R일부 | P4 |
| SYS-03 | NTP 서버·시간 동기 정책·수동 동기 R/W/X/V/B | 설정/동기 성공 구분, 시스템 시간 변경 영향 | R일부 | P5 |
| SYS-04 | LED 트리거·밝기·장치 매핑·버튼 역할 R/W/X/V/B | sysfs 원시 쓰기 금지, 메시지/스크립트는 승인 템플릿만 | R일부 | P5 |
| SYS-05 | 사용자·그룹·로그인 권한·암호 교체 R/W/V/B | shadow/암호 미노출, 계정 참조·복구 접속 보존 | 미구현 | P8 |
| SYS-06 | Dropbear/지원 SSH 서버 접속·인증·포워딩 정책 R/W/X/V/B | 키는 참조, 관리 경로 단절 방지, 실제 새 연결로 확인 | R일부 | P5 |
| SYS-07 | uHTTPd·LuCI·rpcd 관리 리스너/인증/ACL R/W/X/V/B | 인증서 비밀 제외, MCP 자신의 신뢰 경계/정책 변경 불가 | R일부 | P5 |
| SYS-08 | 공개키 등록/삭제·인증서 상태·접속 키 교체 R/W/V/B | 대상 계정 정확 매핑, 비밀 키/QR 반환 금지, 이전 접속 복구 | 미구현 | P8 |
| SYS-09 | 재부팅·안전 종료 R/X/V | boot ID로 새 부팅 확인, 불명 응답 재실행 금지 | 미구현 | P10 |
| SYS-10 | watchdog·온도·팬·전원/성능 정책 R/W/X/V/B | 보드별 센서·안전 범위, 존재하지 않는 하드웨어 값 합성 금지 | R일부 | P10 |
| SYS-11 | 모듈·검토된 sysctl·시스템 한계 설정 R/W/X/V/B | 임의 커널 키/모듈/프로그램 금지, 재부팅 필요 여부·전역 영향 | 미구현 | P9 |
| SYS-12 | 라우터 logd·로그 수준/버퍼·원격 로그 전송 R/W/X/V/B | MCP 감사 로그와 별도, 외부 수신지·로그 개인정보·재시작 영향 | 미구현 | P5 |

### 3.5 네트워크·라우팅·하드웨어 (NET)

주 카테고리 network. 방화벽·DNS·VPN·서비스와 연결되면 모두 포함한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| NET-01 | 장치·논리 인터페이스·링크·통계·소유 관계 R | netifd 관찰과 완전한 커널/물리 토폴로지 구분 | R일부 | P1 |
| NET-02 | 논리 인터페이스 생성/수정/삭제·활성화 R/W/X/V/B | 안정적 ID, 참조 무결성·연결 재검증 | R일부 | P6 |
| NET-03 | IPv4/IPv6 주소·MTU·DNS 위임·prefix delegation R/W/V/B | 주소/프리픽스 범위·중복·수명, 실제 경로·DNS 확인 | R일부 | P6 |
| NET-04 | 브리지·DSA 포트·분리·STP·멀티캐스트 옵션 R/W/X/V/B | 간접 포트 영향, 루프/격리, BPI-R4 실기기 증거 | R일부 | P6 |
| NET-05 | VLAN·bridge-vlan·802.1Q/802.1ad·tag/PVID R/W/V/B | 중복/태그 충돌, 보호 포트 및 관리 경로 보존 | R일부 | P6 |
| NET-06 | IPv4/IPv6 정적 경로·정책 규칙·테이블 R/W/V/B | 우선순위·게이트웨이·interface 참조, 실제 FIB 별도 관찰 | R일부 | P6 |
| NET-07 | 동적 프로토콜: DHCP/PPPoE/PPP/터널 R/W/X/V/B | 설치된 protocol handler별 계약, 비밀/lease 갱신 부작용 | R일부 | P6 |
| NET-08 | WAN 재연결·DHCP renew/release·인터페이스 up/down X/V/B | 관련 세션/주소 손실 명시, 전역 network restart와 구분 | 미구현 | P6 |
| NET-09 | ARP/NDP·이웃·FDB·실제 경로/주소 R/W/X/V/B | 영구 설정/동적 상태 분리, flush는 범위 제한 실행 | R일부 | P6 |
| NET-10 | SFP·Ethernet 속도/duplex·EEE·PHY·MTK offload R/W/X/V/B | 드라이버별 지원·모듈 상태, Wi-Fi/IRQ/가속 영향 검증 | 미구현 | P6 |
| NET-11 | IGMP/MLD·multicast·IPTV passthrough R/W/X/V/B | lan3 직접/간접 보호, 실제 스트림/연결 불변식 확인 | 미구현 | P6 |
| NET-12 | guest/IoT/관리 구역 생성·격리 복합 작업 R/W/X/V/B | network/firewall/dhcp_dns/wireless 합산, 허용/거절 트래픽 양쪽 시험 | 미구현 | P6 |
| NET-13 | Multi-WAN·pbr/mwan3·SQM/QoS·DSCP R/W/X/V/B | 선택 프로필, 의존 경로와 offload 충돌, failover/대역폭 검증 | 미구현 | P8 |
| NET-14 | WWAN/QMI/MBIM·USB tether·bonding·VRF·고급 터널 R/W/X/V/B | 설치/하드웨어별 하위 계약, SIM/인증 정보 참조, 별도 실제 장치 시험 | 미구현 | P8 |

### 3.6 무선 (WIFI)

주 카테고리 wireless. 비밀 자격 증명은 SEC-03, 주소/격리 변경은 관련 네트워크 카테고리도 요구한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| WIFI-01 | 라디오·밴드·국가·허용 채널·능력 R | 현재 관찰/규제/드라이버 제한 구분, 빈 목록을 지원 없음으로 단정 금지 | R일부 | P1 |
| WIFI-02 | AP/BSS/STA 생성·수정·삭제·네트워크 연결 R/W/X/V/B | wifi-device와 wifi-iface 분리, 관리 중인 링크 보호 | 미구현 | P7 |
| WIFI-03 | 채널·폭·출력·국가·DFS·전력 절약 R/W/X/V/B | 규제 위반 설정/우회 금지, 채널 전환·CAC 대기 확인 | R일부 | P7 |
| WIFI-04 | WPA2/WPA3/SAE/OWE·PMF·PSK 설정 R/W/X/V/B | 비밀 참조·암호 정책, 연결 검증, raw key/QR 없음 | 미구현 | P7 |
| WIFI-05 | 802.1X/EAP·RADIUS·인증서·Enterprise R/W/X/V/B | wpad 빌드 기능과 인증서 목적 확인, credential 미노출 | 미구현 | P7 |
| WIFI-06 | 클라이언트·신호·속도·통계·정확한 대상 R | MAC 노출 권한, 중복/사라짐/수명과 카운터 범위 | R일부 | P7 |
| WIFI-07 | 스캔·survey·채널 전환·연결 해제 X/V | 수동 관찰과 능동 스캔 분리, air-time/접속 단절 영향 | 미구현 | P7 |
| WIFI-08 | 게스트 격리·MAC 정책·VLAN 바인딩 R/W/X/V/B | bridge/isolate와 방화벽의 실제 격리 동시 확인 | 미구현 | P7 |
| WIFI-09 | 로밍·802.11k/v/r·band steering R/W/X/V/B | usteer/DAWN 등 구현별 프로필, 패키지/클라이언트 호환성 | 미구현 | P8 |
| WIFI-10 | MLO·802.11be·mesh·WDS·AP+STA R/W/X/V/B | 장치/드라이버별 하위 계약, 각 링크·복합 토폴로지와 복구 시험 | 미구현 | P7 |
| WIFI-11 | 라디오/BSS enable·disable·reload·WPS 정책 R/W/X/V/B | 재로드 영향 전체 포함, WPS 기본 차단·검토된 보안 프로필만 | 미구현 | P7 |

### 3.7 방화벽·NAT (FW)

주 카테고리 firewall. 명령문·nft 스크립트 입력 대신 형식화된 규칙 모델만 허용한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| FW-01 | 기본 정책·SYN/invalid·소프트/하드 offload R/W/X/V/B | 실제 적용 정책, 공유 자원·가속 충돌 | R일부 | P6 |
| FW-02 | zone·network/device 소속·zone 간 forwarding R/W/V/B | 참조/방향/우회 경로·보호 zone 검증 | R일부 | P6 |
| FW-03 | IPv4/IPv6 규칙·순서·시간·rate limit R/W/V/B | 표현식 제한, 순서 의미 보존, 허용/차단 사후검사 | R일부 | P6 |
| FW-04 | 포트 전달·redirect·DNAT·reflection R/W/V/B | 실제 노출/반사/IPv6 영향, 중복 충돌 | R일부 | P6 |
| FW-05 | SNAT·masquerade·NAT loopback R/W/V/B | 소스/출구 인터페이스 및 보호 서비스 검증 | R일부 | P6 |
| FW-06 | nft set/ipset·원소·시간 제한·동적 공급자 R/W/X/V/B | set 참조/크기/만료·firewall4 대체 시 프로필 분리 | 미구현 | P6 |
| FW-07 | 실제 nft/legacy ruleset·counter·handle R | raw 규칙/주석/비밀 미노출, 구성과 실제 룰 불일치 표시 | 미구현 | P6 |
| FW-08 | conntrack 상태·제한된 삭제·counter reset R/X/V | 세션 개인정보·능동 삭제 권한, 전역 flush 기본 차단 | 미구현 | P6 |
| FW-09 | compile/check·reload·재시작 X/V/B | 검증 명령의 파일 생성/실행 효과도 검토, 성공≠연결 확인 | 미구현 | P6 |
| FW-10 | 사용자 include·helper·고급 확장 관리 R/W/X/V/B | 임의 스크립트 대신 검토된 템플릿/프로필, 알 수 없는 전역 영향 거부 | 미구현 | P8 |

### 3.8 DNS·DHCP·RA (DNS)

주 카테고리 dhcp_dns. 고객 식별자/이름/검색 도메인 공개 범위는 개별 읽기 계약과 운영자 정책으로 제한한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| DNS-01 | dnsmasq 인스턴스·인터페이스 바인딩·resolver 정책 R/W/X/V/B | 다중 인스턴스 충돌·실제 질의 경로, 원시 추가 설정 차단 | R일부 | P6 |
| DNS-02 | DHCPv4 pool·범위·lease·정적 reservation R/W/V/B | 네트워크/주소 중복·제외 범위·lease 재사용 검증 | R일부 | P6 |
| DNS-03 | DHCPv6·RA·SLAAC·NDP relay/proxy R/W/X/V/B | 모드·prefix·수명·downstream 실제 IPv6 연결 확인 | R일부 | P6 |
| DNS-04 | A/AAAA·도메인·CNAME·SRV·TXT·검색 도메인 R/W/V/B | 레코드별 타입/범위·루프·공개 범위, 옵션 추가는 개별 검토 | R일부 | P6 |
| DNS-05 | lease/예약/할당 상태·정확 대상·제한된 해제 R/X/V | 관찰된 lease와 현재 접속 구분, 식별자 노출·해제 영향 | R일부 | P6 |
| DNS-06 | odhcpd 자체 설정·저장 경로·로그 정책 R/W/X/V/B | 설정 존재≠실행 증거, 파일 내용/lease-trigger 미노출 | R일부 | P5 |
| DNS-07 | DNS upstream·split DNS·구역별 응답·rebinding/DNSSEC R/W/X/V/B | 구역별 허용/차단 질의, DNSSEC/도메인 누출 확인 | R일부 | P6 |
| DNS-08 | 정적 option/tag/class·PXE/TFTP·boot 설정 R/W/X/V/B | 바이너리 option 타입/길이, 승인된 파일 참조, 임의 명령 차단 | 미구현 | P8 |
| DNS-09 | 캐시·서비스 건강 상태·갱신/flush/reload R/X/V/B | 외부 질의/캐시 변경은 실행, 시간·응답 크기 제한 | 미구현 | P6 |
| DNS-10 | Unbound/DoH/DoT 등 다른 resolver 프로필 R/W/X/V/B | 설치 구성요소/암호화 endpoint 신뢰와 bootstrap 순환 검증 | 미구현 | P8 |

### 3.9 서비스·자동 실행 (SVC)

주 카테고리 services와 서비스가 실제로 관리하는 모든 카테고리. 일반 서비스 상태 읽기는 다른 범주의 서비스 이름도 드러낼 수 있다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| SVC-01 | procd 서비스·인스턴스·PID·exit·부팅 활성 상태 R | 명령/환경/비밀 데이터 제외, 실행/활성/건강 상태 분리 | R일부 | P5 |
| SVC-02 | 검토된 서비스 start/stop/reload/restart X/V/B | 스크립트 정체성·실제 영향 검토, 각 서비스 건강 검사 | 미구현 | P5 |
| SVC-03 | 부팅 enable/disable·시작 순서 R/W/X/V/B | 실행 중 상태와 부팅 설정 독립, 재부팅 효과 | 미구현 | P5 |
| SVC-04 | cron/예약 작업 관리 R/W/X/V/B | 임의 셸 대신 승인된 작업 ID·일정, timezone·중복 실행 제한 | 미구현 | P8 |
| SVC-05 | hotplug·버튼·rc.local 작업 관리 R/W/X/V/B | 검토된 템플릿만, 실행 파일/후크 교체는 operator 권한 | 미구현 | P8 |
| SVC-06 | 의존 서비스·트리거·watchdog/respawn R/W/X/V/B | 연쇄 재시작/루프·전역 영향·회복 보장 검사 | 미구현 | P8 |
| SVC-07 | 서비스별 구성 검사·건강·로그 요약 R/X/V | 검사 자체 부작용 분류, 원시 환경/로그 미노출 | 미구현 | P5 |
| SVC-08 | 다중 서비스 조정·작업 취소·부분 실패 복구 X/V/B | 순서가 명시된 계획, 단일 서비스 복구로 전체 복구 주장 금지 | 미구현 | P8 |

### 3.10 패키지·저장소 (PKG)

주 카테고리 packages. 의존 패키지·후크의 system/services 등 영향도 합산한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| PKG-01 | APK/opkg 설치·상태·architecture·버전 인벤토리 R | 각 관리자의 실제 관찰 범위, 페이지 완전성·상태 해석 별도 | R일부 | P1 |
| PKG-02 | 이용 가능 패키지·검색·상세·의존/역의존·파일 소유 R | 고정 저장소 별칭, 제한된 메타데이터·파일 경로 공개 | 미구현 | P9 |
| PKG-03 | 저장소/피드·우선순위·서명 키 정책 R/W/V/B | 신뢰 기준은 운영자 전용, 임의 URL/서명 무시 옵션 금지 | 미구현 | P9 |
| PKG-04 | 저장소 인덱스 갱신·서명/유효기간 검증 X/V | 네트워크/디스크 쓰기·캐시 오염·중간 실패 | 미구현 | P9 |
| PKG-05 | 설치/삭제 계획·의존 폐쇄·충돌·공간·후크 영향 R | 저장소 세대·정확 버전·다운로드 digest 고정 | 미구현 | P9 |
| PKG-06 | 특정 패키지 설치/삭제/재설치 W/X/V/B | maintainer script는 가역 아님, 실제 서비스 검증·불명 결과 조정 | 미구현 | P9 |
| PKG-07 | 선택 업데이트·downgrade·hold·설정 파일 충돌 W/X/V/B | 이전 바이너리 가용성 확인, 일괄 upgrade를 OS 업그레이드 대체로 제공 금지 | 미구현 | P9 |
| PKG-08 | 복구 가능한 패키지 집합·실패 복구 보고 R/W/X/V/B | 설정 복원≠실행 파일/후크 복원, 불가 항목 명시 | 미구현 | P9 |

### 3.11 저장장치·파일·공유 (STO)

주 카테고리 storage. 파일은 운영자 등록 자원/영역과 검토된 형식으로 한정한다.
원시 시스템 파일 읽기/쓰기가 다른 카테고리와 비밀 보호를 우회해서는 안 된다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| STO-01 | block·partition·filesystem·mount·swap·공간 R | LuCI 목록과 실제 전체 장치 인벤토리 구분 | R일부 | P1 |
| STO-02 | fstab global/mount/swap 설정 R/W/V/B | UUID/label/device 충돌·부팅 영향, 경로 별칭 해석 | R일부 | P9 |
| STO-03 | mount/unmount·swap on/off X/V/B | 사용 중/루트/보호 장치 거부, 잔여 작업·연결 영향 | 미구현 | P9 |
| STO-04 | 파일시스템 검사·안전 제거·건강 관찰 R/X/V/B | 읽기 검사도 장치별 부작용 검토, 쓰기 repair 별도 승인 | 미구현 | P9 |
| STO-05 | 파티션/포맷/크기 변경·암호화 볼륨 R/W/X/V | 비가역 유지보수, 승인된 정확 장치·외부 데이터 백업 필요 | 미구현 | P10 |
| STO-06 | extroot/overlay/loop-image·자동 마운트 R/W/X/V/B | 재부팅/공간 고갈/이미지 손상·기존 데이터 복구 시험 | 미구현 | P9 |
| STO-07 | 승인된 비밀 아닌 파일 list/stat/read/전송/원자적 편집 R/W/V/B | 정확 영역·크기·타입·소유권·symlink/TOCTOU 검사, 범용 root 탐색 금지 | 미구현 | P9 |
| STO-08 | 승인된 파일/디렉터리 생성·이동·삭제·접근 권한 R/W/V/B | 실제 대상 확인·복구 가능 삭제 우선, 설정/키/감사 경로 접근 우회 금지 | 미구현 | P9 |
| STO-09 | Samba/NFS 공유·계정 참조·접근 범위 R/W/X/V/B | 저장장치+서비스+방화벽 합산, 실제 허용/거부 접속 시험 | 미구현 | P8 |
| STO-10 | 용량/할당량·retention·USB 장치 운영 R/W/X/V/B | 시스템/사용자 데이터와 백업 보존 정책 분리, 누락 없는 제한 보고 | 미구현 | P9 |

### 3.12 VPN·터널 (VPN)

주 카테고리 vpn + network/firewall/dhcp_dns/services 중 실제 영향 범위.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| VPN-01 | WireGuard interface·peer·endpoint·allowed IP R/W/X/V/B | handshake/transfer와 연결성 구분, private/preshared key 참조 | 미구현 | P7 |
| VPN-02 | Tailscale 로컬 설정·상태·route/exit/DNS 정책 R/W/X/V/B | auth key/로그인 URL 미노출, 외부 컨트롤러 승인은 별도 권한 | 미구현 | P7 |
| VPN-03 | OpenVPN client/server·profile·인증서 R/W/X/V/B | 임의 inline 비밀/스크립트 차단, 버전별 옵션·경로 검토 | 미구현 | P8 |
| VPN-04 | strongSwan/IPsec·SA·IKE·인증서 R/W/X/V/B | 설치 구성·비밀 참조·SA 수명·네트워크 영향 | 미구현 | P8 |
| VPN-05 | VPN route·split DNS·kill switch·지역망 접근 R/W/X/V/B | DNS/IPv4/IPv6 누출과 터널 손실 시 차단 시험 | 미구현 | P7 |
| VPN-06 | peer/client revoke·rekey·connect/disconnect R/W/X/V/B | 기존 세션 종료 영향, 새 자격 증명으로 확인 | 미구현 | P7 |
| VPN-07 | 터널별 건강 검사·복합 failover R/X/V/B | 외부 probe 허용 목록·손실/복구 조건·관리 터널 보호 | 미구현 | P8 |

### 3.13 펌웨어·부팅·복구 (FIRM)

주 카테고리 firmware. system/storage/packages 및 보호 자원의 전역 영향이 있는 유지보수 영역이다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| FIRM-01 | 현재 image·보드·boot media/layout·호환 프로필 R | SD/eMMC/NAND/NOR/NVMe 등 실제 배치 확인, A/B 가정 금지 | R일부 | P10 |
| FIRM-02 | 승인된 image 검색/획득·출처·서명·digest 검증 R/X | 임의 URL/force 금지, 호환성 검사와 출처 인증 별도 | 미구현 | P10 |
| FIRM-03 | 업그레이드 계획·구성/패키지 보존·공간 검증 R | 정확한 image/부트 상태 바인딩, 미보존 데이터와 예상 중단 공개 | 미구현 | P10 |
| FIRM-04 | flash 제출·새 boot 확인·서비스 검증 W/X/V | 제출 후 응답 유실 재시도 금지, 별도 장치 복구 계획 | 미구현 | P10 |
| FIRM-05 | 설정 factory reset·복원·실패 부팅 복구 W/X/V/B | reset과 firmware 복구 분리, 외부 복구 수단·암호문 백업 확인 | 미구현 | P10 |
| FIRM-06 | 검증된 alternate-slot·boot 선택·업그레이드 롤백 W/X/V | 해당 보드/bootloader 지원과 실제 전원 손실 시험 필요 | 미구현 | P10 |
| FIRM-07 | bootloader·partition table·calibration 유지보수 R/W/X/V | 보드별 별도 고위험 계약/명시적 승인, 일반 sysupgrade 지원으로 계산 금지 | 미구현 | P10 |

### 3.14 진단·관찰·실행 (DIAG)

주 카테고리 diagnostics. 수집 대상의 다른 카테고리도 필요하며 민감한 패킷/로그/프로세스는 별도로 제한한다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| DIAG-01 | 시스템/커널/서비스 로그의 제한된 구조화 조회 R | payload/인수/자격 증명 사전 제외, 원시 logread/dmesg 덤프 금지 | 미구현 | P5 |
| DIAG-02 | 프로세스·소켓·FD·메모리/CPU·부하 R | argv/env 제외, PID 재사용·개인 endpoint·표본 시점 | 미구현 | P5 |
| DIAG-03 | ping·traceroute·DNS·TCP/TLS·연결 검사 X/V | 목적지/포트/빈도/크기 제한, SSRF·DNS rebinding·증폭 방지 | 미구현 | P6 |
| DIAG-04 | 제한된 패킷 요약/카운터·트래픽 관찰 R/X | 원시 패킷/자격 증명 미노출, capture buffer/time/인터페이스 제한 | 미구현 | P6 |
| DIAG-05 | bandwidth/throughput·무선 survey 등 능동 시험 X/V | 외부 비용·트래픽·라우터 부하·보호 경로 영향, 동시 실행 제한 | 미구현 | P7 |
| DIAG-06 | 검토된 프로세스 종료·캐시/카운터 초기화 X/V | 정확 자원·재사용 검증, 시스템 보호·비가역 세션 손실 명시 | 미구현 | P8 |
| DIAG-07 | 이벤트/상태 변화·추가 관찰·문제 설명 R | 온디맨드 기본, subscription 도입은 별도 구조/큐/유실 계약 | 미구현 | P11 |
| DIAG-08 | 안전한 진단 묶음·설정 차이·건강 보고 R/X | 보고서는 비민감 allowlist만, 비밀 포함 자료는 암호문 별도 경로 | 미구현 | P11 |

### 3.15 기준 장치의 부가 서비스 및 선택 프로필 (ADD)

각 행은 services만이 아니라 명시한 도메인 권한을 함께 요구한다. `선택`은 제외가 아니라 설치/적용 여부를 확인할 범위다.
아래 설치 이력은 [기준 기록](reference-target.md)에만 근거한다. 이 문서에서 현재 설치 여부를 새로 확인하지 않았다.

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| ADD-01 | adblock 목록·설정·통계·예외·갱신 R/W/X/V/B | 기록된 패키지; dhcp_dns/services, feed 신뢰·차단/복원 질의 | 미구현 | P8 |
| ADD-02 | ACME 인증서 목록·발급·갱신·배치 R/W/X/V/B | 기록된 패키지; system/services 및 배치 대상, DNS API 비밀·CA rate limit | 미구현 | P8 |
| ADD-03 | Avahi/mDNS reflector·인터페이스·서비스 노출 R/W/X/V/B | 기록된 패키지; network/dhcp_dns/services, 구역 간 정보 노출 시험 | 미구현 | P8 |
| ADD-04 | nginx virtual host·TLS·proxy·접근 정책 R/W/X/V/B | 기록된 패키지; system/services/firewall, 임의 include/실행 설정 금지 | 미구현 | P8 |
| ADD-05 | Samba·Tailscale 운영 조합 R/W/X/V/B | 기록된 패키지; STO-09/VPN-02 재사용, NAS/VPN 접근 허용·거부 통합 시험 | 미구현 | P8 |
| ADD-06 | DDNS·UPnP/NAT-PMP/PCP·동적 포트 정책 R/W/X/V/B | 선택; dhcp_dns/firewall/services, 외부 노출/계정/자동 갱신 영향 | 미구현 | P8 |
| ADD-07 | NFS/FTP·프린터·USB 서비스 R/W/X/V/B | 선택; storage/services/firewall, 사용자 데이터·접근 권한 보호 | 미구현 | P8 |
| ADD-08 | 컨테이너·프록시·메시 라우팅·기타 설치 서비스 R/W/X/V/B | 선택; 패키지별 하위 명세 필수, 임의 이미지/privileged 실행은 우회 기능 아님 | 미구현 | P8 |
| ADD-09 | 알려지지 않은 패키지·사용자 정의 서비스 편입 | 표면 대장에 누락 등록 → 분류 → ADR 필요성 → 타입 계약 → 증거, 자동 승인 없음 | 기반 | P1 |

### 3.16 품질·배포·완료 판정 (QUAL)

| ID | 기능 / 목표 동작 | 추가 수락 기준 | 현황 | 단계 |
| --- | --- | --- | --- | --- |
| QUAL-01 | Rust 계층·디렉터리·의존성·새 경계 진화 하네스 | 아키텍처-only 커밋 선행, production/test 예외 혼동·우회 거부 | 기반 | P2 |
| QUAL-02 | 기능 ID → 세부 계약 → 실제 catalog → 테스트/증거 추적 | 중복 도구 대장을 하네스에 하드코딩하지 않음, 고아·미명세 옵션 검출 | 미구현 | P1 |
| QUAL-03 | Windows GNU/MSVC·Linux·macOS 네이티브 수락 | WSL을 Windows로, cross-build를 실행 증거로 계산 금지 | 기반 | P11 |
| QUAL-04 | APK/opkg·기준/이전/변형·미설치·ACL 제한 시험 | API 형식/패키지 변경 시 부적합 거부 및 계약별 지원 범위 표시 | 기반 | P11 |
| QUAL-05 | parser/정책/작업 상태 fuzz·경합·장애 주입·보안 검토 | 권한 우회·비밀·공급망·전원/연결 손실·자원 고갈, 중대 미해결 결함 없음 | 기반 | P11 |
| QUAL-06 | CPU/RSS/크기·p50/p95/p99·대량 인벤토리·연속 작업 | 지정 환경 반복 측정, 제한/동시성/메모리 회수 검증, 추정치와 분리 | 기반 | P11 |
| QUAL-07 | OpenWrt SDK/musl 패키징·procd 설치·제거·배포 문서 | 호스트 서버와 장치 helper 구분, 서명/SBOM/라이선스·재현 가능한 릴리스 | 미구현 | P11 |
| QUAL-08 | BPI-R4 실기기 기능 대장 폐쇄·보호 기능·복구 수락 | 별도 승인·신선한 토폴로지·대역외 복구, 해당 없음 근거, 미지원 0개 | 미구현 | P11 |

## 4. 기존 도구와 기능 ID의 기준선 대응

현재 55개 도구를 중복 없이 연결한 대응표다. 최초 51개에 P1 조회 4개를 추가했다. 이 대응은 행 전체의 완료나 향후 쓰기 권한을 의미하지 않는다.
도구 명칭/권한의 최종 근거는 실행 파일의 오프라인 catalog와 features/core 계약이다.
예를 들어 기존 watchdog 조회의 실제 카테고리는 diagnostics다. 아래 SYS 연결만으로 system으로 재분류하지 않는다.

| 기능 ID | 현재 기본 도구 이름 |
| --- | --- |
| SYS-01 | `system_board`, `system_info` |
| SYS-02 | `system_configuration` |
| SYS-03 | `system_timeserver_configuration` |
| SYS-04 | `system_led_configuration` |
| SYS-06 | `system_dropbear_configuration` |
| SYS-07 | `system_uhttpd_configuration` |
| SYS-10 | `diagnostics_watchdog_status` |
| NET-01 | `network_device_status`, `network_lan_status`, `network_wan_status`, `network_interface_status`, `network_interfaces` |
| NET-02 | `network_interface_configuration` |
| NET-03 | `network_interface_addresses` |
| NET-04 | `network_device_configuration` |
| NET-05 | `network_bridge_vlan_configuration` |
| NET-06 | `network_route_v4_configuration`, `network_route_v6_configuration`, `network_rule_v4_configuration`, `network_rule_v6_configuration`, `network_interface_routes` |
| NET-09 | `network_interface_neighbors` |
| WIFI-01 | `wireless_radio_info`, `wireless_devices`, `wireless_radio_configuration`, `wireless_countries` |
| WIFI-06 | `wireless_stations`, `wireless_station_status` |
| FW-01 | `firewall_defaults_configuration` |
| FW-02 | `firewall_zone_configuration`, `firewall_forwarding_configuration` |
| FW-03 | `firewall_rule_configuration` |
| FW-04 | `firewall_redirect_configuration` |
| FW-05 | `firewall_nat_configuration` |
| DNS-01 | `dhcp_dnsmasq_configuration` |
| DNS-02 | `dhcp_pool_configuration`, `dhcp_host_configuration` |
| DNS-04 | `dhcp_domain_configuration`, `dhcp_cname_configuration` |
| DNS-05 | `dhcp_v4_leases`, `dhcp_v6_leases` |
| DNS-06 | `dhcp_odhcpd_configuration` |
| DNS-07 | `dhcp_interface_dns` |
| SVC-01 | `service_logd_status`, `service_sysntpd_status`, `service_status`, `service_status_list` |
| PKG-01 | `packages_apk_installed`, `packages_opkg_status` |
| STO-01 | `storage_mounts`, `storage_block_devices` |
| STO-02 | `storage_mount_configuration`, `storage_global_configuration`, `storage_swap_configuration` |

현재 내부 기반은 별도 증거를 참조한다: [권한/기능 발견](capabilities.md),
[키·age](key-management.md), [영향 그래프](protected-resource-effects.md),
[archive](backup-archive-validation.md), [gzip](gzip-archive-validation.md),
[제공 스트림 암호화](validated-archive-sealing.md), [호스트별 보안](platform-support.md).
하나의 기반 모듈이 여러 ID를 보조해도 완성된 관리 기능으로 여러 번 세지 않는다.

## 5. 위험별 완료 기준

| 작업 종류 | 필요한 추가 조건 |
| --- | --- |
| 수동 조회 R | 부작용 검토, 출처/범위·실제 타입/크기·개인정보 공개 범위, 적절한 신선도 |
| 능동 진단 X | 외부 트래픽/부하/목적지 허용, 기간·빈도 제한. 설정을 바꾸지 않는 순수 probe는 구성 백업/롤백 대상이 아님을 계약에 명시 |
| 영속 설정 W 및 서비스 상태 변경 X | 계획·사전 암호문 백업·관련 자원 보호·복구 준비·구성/실제 상태 검증. W만 있는 staging도 소유/충돌/보존 계약 필요 |
| 네트워크/관리 접속 변경 | 장치 측 복구가 SSH와 독립적으로 작동, 새로운 세션/트래픽으로 검증, lan3 간접 영향 차단 |
| 패키지·파일시스템·펌웨어 | 비가역 효과·실제 데이터 백업 범위·대역외 복구·개별 유지보수 승인, 실패 후 상태를 정확히 보고 |
| 비밀 값 변경 | SEC-03의 목적별 참조, 보호된 전송/메모리, 이전 자격 증명 복구 가능성·폐기 순서 |

단일 기능의 완료 조건:

1. 해당 프로필의 모든 적용 가능한 조회/설정/실행 하위 표면과 비밀 대체 경로가 명세되어 있다.
2. 각 하위 표면이 실제 구현 계약·버전·권한·효과·테스트에 연결된다. 부분 기능은 부분 상태를 유지한다.
3. 정상·권한 거부·교차 카테고리·오류·크기·경합·결과 불명·복구 실패 시험을 통과한다.
4. shared 코드는 세 호스트에서 동작하고, 네이티브 시설은 실제 플랫폼에서 주장한 보호 기능을 검증한다.
5. API/실행 가능한 사용자 공간 동작은 에뮬레이터에서, DSA/SFP/무선/flash/boot/보호 서비스는 해당 실기기에서 검증한다.
6. 모든 변경은 실제 사후 상태를 재확인한다. 적용/복구 실패를 성공으로 바꾸거나 자동 재시도하지 않는다.
7. 문서·CLI 안내·catalog·지원 상태·측정·릴리스 제한 사항이 일치한다.

기본 장치에 없는 선택 기능도 ID는 유지한다. 다른 지원 프로필에 편입되면 해당 프로필에 대해 이 기준을 충족한다.
외부 컨트롤러의 계정/ACL·클라우드 DNS 변경은 라우터 권한에 포함되지 않는다. 필요 시 별도 권한을 요구하거나 수동 단계로 명시한다.
HTTP MCP, 다중 사용자 인증, 웹 UI, 모든 암호 알고리즘/컨테이너 형식 구현은 현재 전체 라우터 관리의 필수 조건이 아니다.
그 확장 역시 별도 요구·보안 설계가 필요하며, 공통 호스트 경로나 기본 age/직접·ZIP 키 지원을 대체하지 않는다.

## 6. 설계 근거와 출처

여기서의 기능 목록·권한·단계는 프로젝트의 개발 요구다. 아래 출처는 장치 동작의 제약을 검토하는 근거이지 구현 증거가 아니다.

- 공식 [사용자 가이드](https://openwrt.org/docs/guide-user/start)의 시스템·네트워크·무선·저장장치·추가 소프트웨어 범주를 관리 표면 대조에 사용한다. 이번 검토에서는 검색 색인의 범주를 확인했으며 Wiki 본문 직접 열기는 봇 방지 화면으로 제한됐다.
- 세션/변경 접수/복구의 기존 소스 검토는 [변경 작업 설계안](management-workflows.md)에 보존했다. [기준 rpcd UCI 상수](https://github.com/openwrt/rpcd/blob/e37ed9d814699098eb7e26c8b33c054840782dfb/include/rpcd/uci.h)의 메모리 타이머/런타임 스냅샷은 재부팅을 견디는 암호화 복구와 다르다.
- [25.12.5 rc.common](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/etc/rc.common)의 서비스 실행·reload·boot enable 동작은 별도 효과로 취급한다. 일반적인 종료 코드만으로 건강 상태를 검증하지 않는다.
- [25.12.5 sysupgrade](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/sbin/sysupgrade)의 설정 아카이브·보존 옵션과 직접 복원 경로를 전체 디스크/패키지 복원으로 해석하지 않는다. 후크·공유 임시 파일도 백업 어댑터 검토 범위다.
- [기준 장치 기록](reference-target.md)은 설치 수정본·패키지 및 보드별 차이를 포함한다. 최신 일반 문서나 출시 버전 문자열만으로 설치된 API 호환성을 인정하지 않는다.
