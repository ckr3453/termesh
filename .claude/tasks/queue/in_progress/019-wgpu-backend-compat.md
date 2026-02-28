# wgpu 백엔드 분기 (Vulkan/DX12 테스트, 셰이더 호환)

- phase: 2
- size: M
- blocked_by: 018-pty-windows-compat

## 목표
- wgpu 백엔드(Metal/Vulkan/DX12)별 렌더링 정상 동작 확인
- 셰이더 호환성 및 폴백 처리

## 완료 기준
- [ ] macOS Metal 백엔드 렌더링 정상
- [ ] Windows DX12/Vulkan 백엔드 렌더링 정상
- [ ] Linux Vulkan 백엔드 렌더링 정상
- [ ] GPU 미지원 환경에서 소프트웨어 렌더러 폴백
- [ ] 백엔드별 성능 벤치마크 기록
