import type { Json } from "./driver.ts";

export type NotesBottomResizeValidation = {
  pass: boolean;
  failures: string[];
};

export function validateNotesBottomResizeReceipt(
  receipt: Json,
): NotesBottomResizeValidation {
  const failures: string[] = [];
  const require = (condition: boolean, message: string) => {
    if (!condition) failures.push(message);
  };

  require(receipt?.schemaVersion === 1, "schemaVersion must be 1");
  require(receipt?.disposition === "EVALUABLE_PASS", "disposition must be EVALUABLE_PASS");
  require(receipt?.edgeTrial?.pass === true, "grow edge trial must pass");
  require(
    Number(receipt?.edgeTrial?.distinctHeights) >= 4,
    "grow edge trial must contain at least four distinct heights",
  );
  require(receipt?.shrinkTrial?.pass === true, "shrink edge trial must pass");
  require(
    Number(receipt?.shrinkTrial?.distinctHeights) >= 4,
    "shrink edge trial must contain at least four distinct heights",
  );

  const expectedButtons = receipt?.resizedFooterHitRegions?.regions;
  const buttonTrials = receipt?.buttonTrials;
  require(Array.isArray(expectedButtons), "resized footer inventory is missing");
  require(Array.isArray(buttonTrials), "button trials are missing");
  if (Array.isArray(expectedButtons) && Array.isArray(buttonTrials)) {
    require(
      buttonTrials.length === expectedButtons.length,
      "every resized footer region must have one button-origin trial",
    );
    for (const region of expectedButtons) {
      const trial = buttonTrials.find(
        (candidate: Json) =>
          candidate?.region?.group === region?.group
          && candidate?.region?.index === region?.index,
      );
      require(Boolean(trial), `missing button trial for ${region?.group}/${region?.index}`);
      if (!trial) continue;
      require(trial?.pass === true, `button trial ${region?.index} did not pass`);
      require(
        trial?.route?.route === "protectedFooterButton",
        `button trial ${region?.index} did not route through the protected button path`,
      );
      require(
        trial?.noFrameChange === true,
        `button trial ${region?.index} changed the Notes frame`,
      );
      require(
        trial?.noAction === true,
        `button trial ${region?.index} activated an action`,
      );
      require(
        trial?.result?.untaggedInputCount === 0,
        `button trial ${region?.index} observed untagged input`,
      );
    }
  }

  require(receipt?.persistence?.pass === true, "resized bounds did not persist");
  require(receipt?.cleanedUp === true, "sandbox app did not clean up");
  require(
    Array.isArray(receipt?.topology?.visibleNotesOwners)
      && receipt.topology.visibleNotesOwners.length === 1,
    "final topology must contain exactly one visible Notes owner",
  );
  require(
    Array.isArray(receipt?.screenshots)
      && receipt.screenshots.length >= 5
      && receipt.screenshots.every(
        (entry: Json) =>
          typeof entry?.path === "string"
          && typeof entry?.sha256 === "string"
          && entry.sha256.length === 64,
      ),
    "five hashed screenshots are required",
  );
  require(
    receipt?.edgeTrial?.result?.untaggedInputCount === 0
      && receipt?.shrinkTrial?.result?.untaggedInputCount === 0,
    "edge trials observed untagged input",
  );

  return { pass: failures.length === 0, failures };
}
