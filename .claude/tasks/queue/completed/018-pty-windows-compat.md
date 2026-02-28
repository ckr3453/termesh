# PTY Windows 호환 (ConPTY 분기, 경로 처리)

- phase: 2
- size: M

## 목표
- Windows ConPTY와 Unix PTY 간 플랫폼 분기 처리
- 경로 구분자, 셸 감지 등 플랫폼별 차이 추상화

## 완료 기준
- [x] Windows에서 ConPTY로 cmd.exe / PowerShell 스폰 가능
- [x] Unix에서 기존 portable-pty 동작 유지
- [x] 플랫폼별 기본 셸 자동 감지
- [x] 경로 처리 유틸리티 (구분자, 홈 디렉토리 등)
- [x] Windows/Unix 양쪽 유닛 테스트 통과

## 완료일: 2026-03-01
