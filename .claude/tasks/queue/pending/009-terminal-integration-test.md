# 기본 터미널 통합 테스트 (iterm2 대체 가능 수준)

- phase: 1
- size: M
- blocked_by: 008-termesh-input

## 목표
- 범용 터미널로서 완전한 기능 검증
- 실제 사용 시나리오 테스트

## 완료 기준
- [ ] 터미널에서 `zsh` 실행 가능
- [ ] 입력한 명령어 출력 정상 렌더링
- [ ] 256 color 지원 확인
- [ ] true color (24-bit RGB) 지원 확인
- [ ] 스크롤백 버퍼 동작
- [ ] 윈도우 리사이징 시 터미널 크기 조정
- [ ] Pane 분할 후 각 pane에서 독립 셸 실행
- [ ] 키바인딩 (Cmd+H/J/K/L) 포커스 이동 정상
- [ ] 세션 저장/복원
- [ ] 사용자 문서 작성: 기본 사용법 (README.md)

## 테스트 시나리오
1. 앱 시작 → `zsh` 자동 시작
2. `cd /tmp && ls -la` 실행
3. Cmd+T로 새 pane 생성, 다시 `zsh` 실행
4. 다양한 색상 출력 (lolcat, colorls 등) 렌더링 확인
5. Cmd+Enter로 Focus ↔ Split 모드 전환
6. 앱 종료 → 세션 복원 테스트

## 노트
- 성능: 60fps 유지 확인
- 메모리: 기본 40MB 이하
