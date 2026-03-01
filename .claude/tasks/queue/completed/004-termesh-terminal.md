# termesh-terminal: 터미널 에뮬레이션 (alacritty_terminal 래핑)

- phase: 1
- size: M
- blocked_by: 003-termesh-pty

## 목표
- alacritty_terminal을 Termesh 맞춤으로 래핑
- VT100 이스케이프 시퀀스 처리, 렌더링 가능한 그리드 제공

## 완료 기준
- [ ] `Terminal` 구조체: 터미널 상태, 그리드 관리
- [ ] `feed_bytes()`로 출력 스트림 입력 처리
- [ ] `render_grid()`로 렌더링 가능한 셀 그리드 반환
- [ ] 색상 지원 (256 color, true color)
- [ ] 스크롤백 버퍼 (설정 가능, 기본 10000줄)
- [ ] 단위 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- grid: 셀 기반 그리드 구조체
- vt100: 이스케이프 시퀀스 처리
- color: 색상 정의 및 변환

## 노트
- alacritty_terminal 문서: https://docs.rs/alacritty_terminal/
- 초기 그리드 크기: 80x24
