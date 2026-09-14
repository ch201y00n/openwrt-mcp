# P1 산출물: 첫 변경 관찰·효과·복구 의존성

상태: **P1 시점의 P2 입력 검토 기록**. 아래 후보 결정은 이후
[ADR 0021](adr/0021-guarded-mutation-boundaries.md)과
[P2 계약](guarded-mutation-contracts.md)에서 구체화했다. 과거의 미확정 항목과
현재 결정을 구분한다. 실제 라우터 변경·guardian 설치·키 제공 승인이 아니다.

## D1 후보와 현재 관찰 공백

첫 후보는 SYS-02의 기존 system 섹션에 있는 비밀 아닌 hostname 문자열 하나다.
최종 옵션·유효 값·안정적인 섹션 식별·적용 방식을 P2에서 확정한다.
익명 인덱스, 임의 config/option, raw shell, 전체 system reload를 편의상 선택하지 않는다.

| 필요한 근거 | 현재 재사용 가능 | P2/P3에서 필요한 추가 계약 |
| --- | --- | --- |
| 같은 대상과 부팅 세대 | pinned SSH, system_board, system_info, backend epoch | restart를 넘는 boot/identity·plan binding; uptime만으로 대체 금지 |
| 구성 기준과 충돌 | system_configuration, 고정 UCI 서명 | committed-only baseline, 안정된 section ID, foreign pending·drift 검출 |
| 적용의 전체 효과 | system_timeserver_configuration, 새 LED/SSH/uHTTPd 조회, service 상태 | 설치 system script의 모든 효과, 다른 패키지 hostname 참조·reload 연쇄 |
| 보호 자원 | network/interface/device/VLAN/route/firewall 조회와 순수 영향 모델 | 신선한 before/after 그래프, 공유 서비스·lan3 간접 영향과 unknown 차단 |
| 백업 | archive/gzip/age/sealing 내부 모듈 | 같은 대상 binary capture, manifest/producer 완결, private ciphertext store |
| 적용 후 검증 | 선택된 UCI와 서비스 상태 | 실제 runtime hostname와 committed 상태를 각각 관찰, 새 세션·보호 서비스 확인 |
| 복구 확인 | 일반 조회 기반 | inverse의 출처/대상/범위, recovery job 상태, 구성·실행·보호 상태 재검증 |

현재 UCI 조회는 shared pending delta를 포함할 수 있어 저장 완료 기준으로 사용할 수 없다.
새 네 조회도 LED/SSH/web/odhcpd의 실제 상태나 전체 system 서비스 효과를 증명하지 않는다.
패키지 hostname 참조/설치 script 변경은 현재 미확인이므로 안전하다고 추정하지 않는다.

## D2 장치 실행 접수와 동시 변경

제안: 최초 변경 프로필은 운영자가 확보한 독점 관리 구간에서만 허용하고,
장치 측 admission owner가 여러 MCP 호스트의 충돌을 직렬화한다.
LuCI/CLI/외부 root writer는 그 잠금을 따르지 않을 수 있으므로 외부 writer 격리를
보증하지 않는다. foreign pending/drift가 감지되면 거부하며 타인의 delta를
commit/revert하지 않는다. guardian의 인증·허용 동작·배포·IPC·자원 예산은
P2 ADR와 부정 하네스를 먼저 통과해야 한다. 이 문서는 guardian을 설치하지 않는다.

## D3 복구 권한과 내구성

- archival age 공개 수신자로 암호화하는 능력과 장치의 자율 복구 권한을 분리한다.
- 보관용 age 개인키를 라우터로 복사하지 않는다. 별도 장치 복구 권한/키 제공,
  암호문 저널·boot recovery·저장 실패 조건은 운영자 결정과 P2 설계가 필요하다.
- 메모리만 사용하는 실험 복구는 재부팅/전원 손실 보장이 없다고 명시한다.
  완성된 durable recovery의 대체 구현으로 선택하지 않는다.
- 호스트/SSH 단절, guardian/rpcd 재시작, 재부팅, 저장 실패, 키 불가용,
  confirm/timeout 경합, 제출 후 응답 유실을 분리해 테스트한다.
- v15 영향 모델, v16/v17 codec, v18 sealing을 연결하기 전에 각 생산 소비자
  금지를 필요한 소유자에만 좁혀 확장하는 설계·하네스 체크포인트가 필요하다.

## P2로 넘기는 실행 목록

1. D1의 정확한 scalar·서비스 효과와 D2의 동시 변경 정책을 확정한다.
2. D3 복구 보장·권한·내구성 선택을 검토하고, 미확정 운영 경로/키는 합성 fixture로 분리한다.
3. 목적별 trait·상태 기계·소유자·권한 합산·예산·취소/결과 불명·감사 실패를 ADR/spec에 정의한다.
4. 잘못된 소비자·숨은 I/O·권한 누락·평문 저장·무제한 값·재제출 우회를 부정 하네스로 차단한다.
5. 설계만 검증·커밋한 뒤 P3의 실제 저장/전송 어댑터와 P4의 첫 전체 흐름을 구현한다.

P1 조회는 장치/키 없이 검증할 수 있다. 위 결정이 남았다고 조회를 무한히 늘리지 않고,
다음 단계에서 선택을 확정한다. 실기기 변경·배포·파괴 시험은 항상 별도 승인 범위다.
근거: [변경 설계안](management-workflows.md), [단계별 계획](implementation-plan.md),
[기준 장치 기록](reference-target.md), [관리 표면 대장](management-inventory.md).
