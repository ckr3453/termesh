# Rust 코드 컨벤션

## 파일 & 모듈
- 파일명: `snake_case`
- 모듈: pub 최소화, 필요한 것만 노출
- 한 파일 최대 300줄 원칙

## 네이밍
- **타입**: `PascalCase`
- **함수/변수**: `snake_case`
- **상수**: `SCREAMING_SNAKE_CASE`
- **에러 타입**: `<Action>Error` (e.g., `PtySpawnError`)

## 에러 처리
```rust
// ❌ 금지
fn load_config() -> Result<Config> {
    Ok(parse_toml()?) // 컨텍스트 없음
}

// ✅ 권장
fn load_config() -> Result<Config, ConfigError> {
    parse_toml()
        .context("Failed to parse config.toml")?
}
```

- **`anyhow`** 금지, **`thiserror`** 사용
- 모든 에러는 컨텍스트 메시지 포함
- 공개 API는 커스텀 에러 타입 정의

## 테스트
- 단위 테스트: 모듈 하단 `#[cfg(test)]`
- 통합 테스트: `tests/` 디렉토리
- 테스트명: `test_<function>_<scenario>` (e.g., `test_spawn_pty_with_invalid_cwd`)

## 코드 리뷰
- 태스크 완료 후 커밋 전, `code-reviewer` 서브에이전트로 변경된 파일 리뷰 실행
- 리뷰 관점: 보안 취약점, 에러 처리 누락, pub API 설계, 불필요한 복잡도
- 리뷰에서 발견된 이슈는 같은 커밋에서 수정 (별도 태스크 불필요)

## 문서
- pub 함수/타입은 반드시 doc 주석 포함
- 복잡한 로직은 인라인 주석으로 설명
- 공개 모듈: `//! Module description`
