# 성능 최적화 및 메모리 프로파일링

- phase: 1
- size: M
- blocked_by: 014-split-mode-ui

## 목표
- 터미널 렌더링 및 에이전트 상태 업데이트 성능 최적화
- 메모리 사용량 프로파일링 및 최적화

## 완료 기준
- [ ] 렌더링 프레임 레이트 측정 (목표: 60fps 유지)
- [ ] 메모리 프로파일링:
  - 기본 상태: < 50MB
  - 세션 4개: < 100MB
  - Instruments 또는 heaptrack으로 검증
- [ ] 불필요한 할당 제거 (Arc/Mutex 검토)
- [ ] 이벤트 루프 CPU 사용률 검증 (idle < 5%)
- [ ] 수정 사항 문서화
- [ ] Cargo release 빌드 프로필 최적화
- [ ] `cargo test` 통과

## 최적화 항목
- [ ] 렌더러 더블 버퍼링
- [ ] 변경된 셀만 리드로우
- [ ] 어댑터 상태 추론 캐싱
- [ ] 파일 diff 생성 비동기화

## 노트
- macOS Instruments: `Xcode.app/Contents/Applications/Instruments.app`
- 릴리스 빌드: `cargo build --release`
