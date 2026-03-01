# 이벤트 루프에 App 연동

- phase: 1
- size: M

## 목표
- termesh-platform의 이벤트 루프를 App 구조체와 연결
- InputHandler → Action 디스패치 체계 구축

## 완료 기준
- [ ] platform::run()이 App을 받아서 동작
- [ ] 키보드 입력 → InputHandler → Action 변환
- [ ] Action에 따른 App 상태 변경 (모드 전환, 포커스 이동 등)
- [ ] EventBus를 통한 이벤트 발행
- [ ] 윈도우 리사이즈 시 레이아웃 재계산
- [ ] 유닛 테스트 통과
