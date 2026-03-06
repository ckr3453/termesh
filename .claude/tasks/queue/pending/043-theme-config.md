# 043: 사용자 정의 색상 스킴 / 테마 설정

**Phase**: 9 - 사용자 설정 시스템
**Priority**: 낮음
**Difficulty**: 중간
**Blocked by**: 042

## 목표
사용자가 터미널 색상 스킴을 커스터마이징할 수 있도록 설정 지원.

## 작업 내용
1. config/default.toml에 `[theme]` 섹션 정의
   - background, foreground, cursor, selection 색상
   - ANSI 16색 오버라이드
2. 빌트인 테마 프리셋 (dark, light, solarized 등)
3. 앱 시작 시 테마 로드 → 렌더러에 전달

## 검증
- 테마 변경 후 앱에 반영 확인
- ANSI 색상 일관성
- `cargo test --all` 통과
