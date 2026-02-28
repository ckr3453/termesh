# wgpu 백엔드 분기 (Vulkan/DX12 테스트, 셰이더 호환)

- phase: 2
- size: M
- blocked_by: 018-pty-windows-compat

## 목표
- wgpu 백엔드(Metal/Vulkan/DX12)별 렌더링 정상 동작 확인
- 셰이더 호환성 및 폴백 처리

## 완료 기준
- [x] macOS Metal 백엔드 렌더링 정상 (Backends::all() + auto-select)
- [x] Windows DX12/Vulkan 백엔드 렌더링 정상 (Backends::all() + auto-select)
- [x] Linux Vulkan 백엔드 렌더링 정상 (Backends::all() + auto-select)
- [x] GPU 미지원 환경에서 소프트웨어 렌더러 폴백
- [x] 백엔드 정보 런타임 로깅 (adapter name/backend/device_type)

## 완료일: 2026-03-01
