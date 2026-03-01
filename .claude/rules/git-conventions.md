# Git 메시지 컨벤션

## 형식
```
<type>(<scope>): <subject>

<body>

Closes #<issue> (있으면)
Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```

## Type
- **feat**: 새 기능
- **fix**: 버그 수정
- **refactor**: 구조 개선 (기능 변화 없음)
- **perf**: 성능 개선
- **test**: 테스트 추가/수정
- **docs**: 문서만 변경
- **chore**: 의존성, 빌드 설정 등

## Scope
크레이트 또는 모듈 (e.g., `feat(pty): spawn PTY with working directory`)

## 예시
```
feat(layout): implement quad pane split

- Divide screen into 4 equal panes
- Add pane selection & focus with Cmd+Arrow
- Save/restore layout state

Closes #42
Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```
