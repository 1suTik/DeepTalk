import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { MAX_PROFILES, ProfileLibraryPage, type ProfileDoc } from "./ProfileLibraryPage";

function makeProfile(overrides: Partial<ProfileDoc> = {}): ProfileDoc {
  return {
    id: "p-1",
    title: "会议助手项目简介",
    originalPath: "C:\\docs\\sample.md",
    importedAtMs: 1_700_000_000_000,
    enabled: true,
    ...overrides,
  };
}

describe("ProfileLibraryPage", () => {
  it("shows empty state when no profiles", () => {
    render(<ProfileLibraryPage profiles={[]} />);
    expect(screen.getByText(/尚未导入资料/)).toBeVisible();
    expect(screen.getByLabelText("资料数量")).toHaveTextContent("0/10");
  });

  it("lists imported profiles with enable toggle", () => {
    const onToggle = vi.fn();
    render(
      <ProfileLibraryPage
        profiles={[makeProfile(), makeProfile({ id: "p-2", title: "岗位说明", enabled: false })]}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("会议助手项目简介")).toBeVisible();
    expect(screen.getByText("岗位说明")).toBeVisible();
    const toggles = screen.getAllByRole("button", { name: /启用中|已停用/ });
    expect(toggles[0]).toHaveTextContent("启用中");
    expect(toggles[1]).toHaveTextContent("已停用");
    fireEvent.click(toggles[0]);
    expect(onToggle).toHaveBeenCalledWith("p-1", false);
  });

  it("removes a profile", () => {
    const onRemove = vi.fn();
    render(<ProfileLibraryPage profiles={[makeProfile()]} onRemove={onRemove} />);
    fireEvent.click(screen.getByRole("button", { name: "移除 会议助手项目简介" }));
    expect(onRemove).toHaveBeenCalledWith("p-1");
  });

  it("disables import at the limit", () => {
    const profiles = Array.from({ length: MAX_PROFILES }, (_, i) =>
      makeProfile({ id: `p-${i}` }),
    );
    render(<ProfileLibraryPage profiles={profiles} />);
    expect(screen.getByTestId("import-button")).toHaveTextContent("已达上限（10 份）");
    expect(screen.getByLabelText("资料数量")).toHaveTextContent("10/10");
  });
});
