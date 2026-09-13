# P1 관리 표면 대장과 지원 프로필

이 문서는 [기능 명세](management-feature-spec.md)의 CTL-03~07/10, PKG-01,
QUAL-02, ADD-09와 필수 NET-01/STO-01/WIFI-01 관찰을 추적한다.
기계 판독 대장은 [management-surfaces.toml](../compatibility/management-surfaces.toml)이다.
**저장소 개발용 기록이며 런타임 설정·권한·자동 탐지 결과가 아니다.**
현재 관리 도구 55개를 연결하지만 모든 기능 ID의 구현 완료를 뜻하지 않는다.

## 기록 형식과 소유권

기존 compatibility 문서 영역에 버전 1 TOML 자료를 두고, 기존 필수
`features/tests/capability_contracts.rs`의 `inventory` 하위 모듈에서 검증한다.
새 생산 포트·의존성·실행 권한·하네스 예외를 만들지 않는다. 따라서 v20의
새 체크포인트 없이 기존 개발/기능 계약 경계 안에서 추가할 수 있다.
xtask에 도구 목록을 복제하지 않고 실제 features catalog와 문서 대응표를 대조한다.
향후 실시간 inventory 도구·새 수집 포트·새 프로필 선택 권한은 별도 설계 사항이다.

| 항목 | 내용 |
| --- | --- |
| schema_version / snapshot | 대장 형식과 검토된 개발 증분 식별자; 장치 부팅 세대가 아님 |
| scope / whole_device_complete | `recorded_not_live` / false 고정; 실행 중 장치 전체성을 주장하지 않음 |
| profiles | 기존 evidence.toml 대상 ID·역사적 관찰 범위·출처. 실제 신원/버전은 원본을 참조 |
| surfaces | 기능 ID, 고정 도구 참조, 선택자/세부 계약 참조, R 범위, 일부 구현 상태, 노출 정보·위험, 증거, 남은 공백 |
| observations | 기록된 패키지·객체·메서드 묶음 또는 미확인 CLI/하드웨어 표면, 대상 프로필, 기능 ID, 위험, 출처, 공백 |

자료에는 설정 값·실제 endpoint·키·자격 증명·MAC·원시 UCI·전체 패키지 DB를
넣지 않는다. 선택자는 개발용 식별 문자열일 뿐 호출 가능한 argv/경로가 아니다.
현재 55개 도구는 정확히 한 surface에 연결하며, 임의 미래 기능은 도구로 생성하지 않는다.
`risk`는 공개 정보와 오판/공유 효과의 위험을 기록한다. 기록용 설명이며 런타임 권한 비트가 아니다.

## 상태와 증거

- surface `partial`: 해당 ID의 선택된 R 계약만 구현. 변경/실행/검증/복구 완료 아님.
- observation `recorded`: 원본에 이름·일부 버전 또는 API 메타데이터가 기록됨.
  설치 사실과 성공 호출·현재 가용성은 서로 다르다.
- observation `unknown`: 세부 자료가 없거나 확인되지 않음. 미설치/absent/지원 완료로 바꾸지 않는다.
- profile `historical`: 원본 시점의 부분 관찰. 현재 대상의 신선한 인벤토리 아님.
- 출처 문서, 독립 fixture, 호스트 실행, emulator, exact-device 증거를 섞지 않는다.
  기존 수락 범위는 [evidence.toml](../compatibility/evidence.toml)에 그대로 남는다.

판독 검사는 형식/알 수 없는 필드, 중복 ID/도구/토큰, 존재하지 않는 기능/프로필,
잘못된 증거 경로, 잘못된 R 범위/상태, 실제 catalog 누락·유령 도구,
기능 명세 대응표 불일치, 기준 기록의 패키지·객체·메서드 누락을 거부한다.
이 검사는 문서 추적의 정합성을 보장할 뿐 실제 장치 자료의 진위를 증명하지 않는다.

## 고정한 비교 프로필

| 프로필 | 사용 범위 | 확인되지 않은 부분 |
| --- | --- | --- |
| bpi-r4-openwrt-25.12.5-reference | 기준 장치의 2026-09-13 기록, APK 3.0.5, 선택 패키지/API | 전체 패키지 manifest·옵션·드라이버·현재 장치 상태 |
| qemu-arm64-openwrt-25.12.5 | 별도 ARM64 userspace/APK 계약 시험 이력 | BPI-R4 DSA/SFP/Wi-Fi/boot 등 하드웨어 |
| qemu-arm64-openwrt-24.10.4 | 별도 opkg root-status 계약 시험 이력 | 추가 destination/layer, 기준 장치 설치 사실 |

버전은 각 원본 target 기록에 고정한다. 같은 release라도 rpcd/package revision과
downstream 변경이 다르면 동일 프로필이라고 자동 판단하지 않는다.
범위 밖 버전·포크·드라이버·설치 패키지는 `unknown` 공백으로 추가하고 검토 후 확장한다.

## 버전 차이와 변경 감지 절차

1. 대상/보드/부팅 세대, package revision, API signature, UCI 옵션/형식,
   driver/CLI 표면을 서로 다른 증거로 수집한다. 허용되지 않은 원시 값은 수집하지 않는다.
2. 기준 프로필과 차이를 기록한다. 기존 런타임은 같은 backend epoch의 짧은
   UCI/API 관찰 캐시와 고정 APK/opkg 프로필만 검증한다. 전역 drift 탐지기는 아직 없다.
3. ACL-hidden/관찰 실패는 unknown, 실제 서명 충돌은 incompatible로 유지한다.
   APK 실패 후 opkg 자동 재시도, 버전 문자열 기반 허용, 동적 명령 생성은 하지 않는다.
4. 신원/부팅/패키지/옵션 차이를 발견하면 이전 수락을 새 대상에 승계하지 않는다.
   계획·복구에 사용할 영속 기준 바인딩과 전역 변경 감지는 P2 이후의 별도 구현이다.
5. P1은 필수 관찰을 정리하는 단계다. 모든 조회를 끝내기 전에 첫 변경의 최소
   관찰·영향·복구 조건을 확정하고 [P2 진입 검토](first-mutation-readiness.md)로 진행한다.

## 명시적으로 남긴 공백

| 공백 ID | 미확인 범위 |
| --- | --- |
| `unretained-object-headers` | 기록하지 않은 ubus 객체/메서드 |
| `complete-package-manifest` | 선택 패키지 표 밖의 설치 구성요소 |
| `all-uci-options` | 전체 섹션/옵션과 downstream 변경 |
| `non-uci-cli` | 비 UCI 파일, CLI, cron/hotplug/패키지 후크 |
| `network-hardware` | DSA/SFP/VLAN·보호 IPTV 의존성 |
| `wireless-driver` | 실제 driver/firmware/MLO·규제 조건 |
| `boot-storage` | boot media, 파티션, extroot·대역외 복구 |
| `first-change-effects` | 첫 변경의 pending·소비자·reload 영향 |
| `external-authority` | DNS/CA/Tailscale/저장소 외부 권한 |

기준 기록은 60개 visible ubus object 중 일부 이름만 보존했다. 나머지를 추정하지 않는다.
선택 패키지 표 이외의 설치 패키지, 전체 UCI 옵션, 비 UCI 설정/CLI/후크,
DSA/SFP/무선 driver/MLO/boot media, 외부 서비스 권한을 각각 unknown으로 추적한다.
현재 목록·주소·mount·radio 조회는 전체 자원 그래프나 변경 기준이 아니다.

신선한 실기기 자료가 필요할 때에는 별도 범위를 확인하고, 우선 board의 비민감
신원, 선택 package metadata, 제한된 API 서명, 타입/옵션 이름의 민감도 검토를
제시한다. 원시 UCI나 비밀·실제 파일 내용·개인 Vault는 이 개발 시험에 필요하지 않다.
현재 P1은 기록된 표면을 ID 또는 explicit unknown에 연결하는 완료 기준이며,
CTL/PKG/NET/STO/WIFI 기능군 전체 지원이나 실기기 수락 완료가 아니다.
