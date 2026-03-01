# Cargo workspace 설정

- phase: 1
- size: S
- blocked_by:

## 목표
- 루트 `Cargo.toml` 설정
- 11개 크레이트의 워크스페이스 구조 생성

## 완료 기준
- [ ] `Cargo.toml`에 workspace.members 정의
- [ ] `crates/` 디렉토리에 각 크레이트 폴더 생성
- [ ] `cargo check` 성공

## 노트
- termesh-core부터 termesh-app까지 11개 크레이트
- 각 크레이트의 `Cargo.toml`은 추후 작성
