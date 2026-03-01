# 파일 워처 기동 + diff 사이드 패널 연결

- phase: 5
- size: M
- blocked_by: 040-side-panel-render

## 목표
- 앱 시작 시 FileWatcher를 생성하고 변경된 파일의 diff를 사이드 패널에 표시

## 완료 기준
- [ ] 워크스페이스 디렉토리를 감시하는 FileWatcher 생성
- [ ] 파일 변경 감지 시 diff_generator로 diff 생성
- [ ] 생성된 diff가 사이드 패널의 Diff 탭에 반영
- [ ] .git, node_modules 등 무시 디렉토리 필터링 동작
