# termesh-diff: 파일 감시 (notify), diff 생성 (similar)

- phase: 1
- size: M
- blocked_by: 010-termesh-agent

## 목표
- 워크스페이스 파일 변경 감시
- 에이전트가 수정한 파일의 diff 생성 및 표시

## 완료 기준
- [ ] `FileWatcher` 구조체: notify 기반 파일 감시
- [ ] 무시 패턴 지원 (.git, node_modules, target, __pycache__)
- [ ] 변경 감지 시 이전 버전과 diff 생성 (similar 크레이트)
- [ ] diff 포맷: unified diff (표준)
- [ ] 변경 이력 저장 (최근 100개 변경)
- [ ] 사이드 패널에 diff 데이터 제공
- [ ] 단위 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- watcher: FileWatcher 구현
- diff_generator: similar 래퍼
- history: 변경 이력 관리

## 노트
- 대형 파일 (>10MB) 제외
- 바이너리 파일 감시 안 함
- 초기 파일 내용 캐싱
## 완료: 2026-03-01
