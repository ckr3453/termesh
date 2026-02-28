# 레이아웃 연동 렌더링

- phase: 1
- size: M
- blocked_by: 009b-multi-session-manager

## 목표
- Focus/Split 모드별 pane 영역에 각 세션의 터미널 출력 렌더링

## 완료 기준
- [ ] Focus 모드: 선택된 세션의 터미널을 메인 영역에 렌더링
- [ ] Split 모드: 각 pane 영역에 바인딩된 세션 터미널 렌더링
- [ ] Renderer가 PixelRect 영역 단위로 GridSnapshot 렌더링
- [ ] 모드 전환 시 렌더링 영역 즉시 재계산
- [ ] 유닛 테스트 통과
