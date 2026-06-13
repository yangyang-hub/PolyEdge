"use client";

import type { Dispatch, SetStateAction } from "react";

import { Input } from "@/components/ui/input";
import type {
  RewardAiProvider,
  RewardAiRequestFormat,
  RewardBotConfigDto,
  RewardInfoRiskLevel,
  RewardQuoteMode,
  RewardSelectionMode,
} from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import type { NumberConfigKey } from "../types";
import { NumberInput } from "./number-input";
import { ConfigSection, Hint, ToggleField, selectClassName } from "./rewards-config-fields";

type AdvancedConfigProps = {
  draft: RewardBotConfigDto;
  setDraft: Dispatch<SetStateAction<RewardBotConfigDto>>;
  updateNumber: (key: NumberConfigKey, value: string) => void;
};

export function BookSelectionConfig({
  draft,
  setDraft,
  updateNumber,
}: AdvancedConfigProps) {
  const h = dictionary.rewards.configHints;

  return (
    <ConfigSection
      title={dictionary.rewards.configBookSelection}
      description={dictionary.rewards.configBookSelectionDescription}
    >
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.quoteMode}
          <Hint content={h.quoteMode} />
        </span>
        <select
          className={selectClassName}
          value={draft.quote_mode}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              quote_mode: event.target.value as RewardQuoteMode,
            }))
          }
        >
          <option value="double">{dictionary.rewards.quoteModeDouble}</option>
          <option value="auto">{dictionary.rewards.quoteModeAuto}</option>
        </select>
      </label>
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.selectionMode}
          <Hint content={h.selectionMode} />
        </span>
        <select
          className={selectClassName}
          value={draft.selection_mode}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              selection_mode: event.target.value as RewardSelectionMode,
            }))
          }
        >
          <option value="observe">{dictionary.rewards.selectionObserve}</option>
          <option value="enforce">{dictionary.rewards.selectionEnforce}</option>
        </select>
      </label>
      <ToggleField
        label={dictionary.rewards.dominantSingleSideEnabled}
        hint={h.dominantSingleSideEnabled}
        checked={draft.dominant_single_side_enabled}
        onChange={(checked) =>
          setDraft((current) => ({ ...current, dominant_single_side_enabled: checked }))
        }
      />
      <NumberInput
        label={dictionary.rewards.dominantMinProbability}
        value={draft.dominant_min_probability}
        hint={h.dominantMinProbability}
        onChange={(value) => updateNumber("dominant_min_probability", value)}
      />
      <NumberInput
        label={dictionary.rewards.dominantMaxProbability}
        value={draft.dominant_max_probability}
        hint={h.dominantMaxProbability}
        onChange={(value) => updateNumber("dominant_max_probability", value)}
      />
      <NumberInput
        label={dictionary.rewards.dominantMinExitDepthUsd}
        value={draft.dominant_min_exit_depth_usd}
        suffix="$"
        hint={h.dominantMinExitDepthUsd}
        onChange={(value) => updateNumber("dominant_min_exit_depth_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.maxTop1DepthShare}
        value={draft.max_top1_depth_share}
        hint={h.maxTop1DepthShare}
        onChange={(value) => updateNumber("max_top1_depth_share", value)}
      />
      <NumberInput
        label={dictionary.rewards.maxTop3DepthShare}
        value={draft.max_top3_depth_share}
        hint={h.maxTop3DepthShare}
        onChange={(value) => updateNumber("max_top3_depth_share", value)}
      />
      <NumberInput
        label={dictionary.rewards.maxBookHhi}
        value={draft.max_book_hhi}
        hint={h.maxBookHhi}
        onChange={(value) => updateNumber("max_book_hhi", value)}
      />
      <NumberInput
        label={dictionary.rewards.preferredCategoryScoreBonus}
        value={draft.preferred_category_score_bonus}
        hint={h.preferredCategoryScoreBonus}
        onChange={(value) => updateNumber("preferred_category_score_bonus", value)}
      />
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.preferredCategories}
          <Hint content={h.preferredCategories} />
        </span>
        <Input
          value={draft.preferred_categories.join(",")}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              preferred_categories: event.target.value
                .split(",")
                .map((item) => item.trim())
                .filter(Boolean),
            }))
          }
        />
      </label>
    </ConfigSection>
  );
}

export function AiAdvisoryConfig({
  draft,
  setDraft,
  updateNumber,
}: AdvancedConfigProps) {
  const h = dictionary.rewards.configHints;

  return (
    <ConfigSection
      title={dictionary.rewards.configAiAdvisory}
      description={dictionary.rewards.configAiAdvisoryDescription}
    >
      <ToggleField
        label={dictionary.rewards.aiAdvisoryEnabled}
        hint={h.aiAdvisoryEnabled}
        checked={draft.ai_advisory_enabled}
        onChange={(checked) =>
          setDraft((current) => ({ ...current, ai_advisory_enabled: checked }))
        }
      />
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.aiProvider}
          <Hint content={h.aiProvider} />
        </span>
        <select
          className={selectClassName}
          value={draft.ai_provider}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              ai_provider: event.target.value as RewardAiProvider,
              ai_request_format:
                event.target.value === "anthropic"
                  ? "anthropic_messages"
                  : current.ai_request_format === "anthropic_messages"
                    ? "openai_responses"
                    : current.ai_request_format,
            }))
          }
        >
          <option value="openai">OpenAI</option>
          <option value="anthropic">Anthropic</option>
        </select>
      </label>
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.aiRequestFormat}
          <Hint content={h.aiRequestFormat} />
        </span>
        <select
          className={selectClassName}
          value={draft.ai_request_format}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              ai_request_format: event.target.value as RewardAiRequestFormat,
            }))
          }
        >
          {draft.ai_provider === "anthropic" ? (
            <option value="anthropic_messages">Anthropic Messages</option>
          ) : (
            <>
              <option value="openai_responses">OpenAI Responses</option>
              <option value="openai_chat_completions">OpenAI Chat Completions</option>
            </>
          )}
        </select>
      </label>
      <NumberInput
        label={dictionary.rewards.aiAdvisoryTtlSec}
        value={draft.ai_advisory_ttl_sec}
        suffix="s"
        hint={h.aiAdvisoryTtlSec}
        onChange={(value) => updateNumber("ai_advisory_ttl_sec", value)}
      />
      <ToggleField
        label={dictionary.rewards.infoRiskEnabled}
        hint={h.infoRiskEnabled}
        checked={draft.info_risk_enabled}
        onChange={(checked) =>
          setDraft((current) => ({ ...current, info_risk_enabled: checked }))
        }
      />
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.infoRiskMode}
          <Hint content={h.infoRiskMode} />
        </span>
        <select
          className={selectClassName}
          value={draft.info_risk_mode}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              info_risk_mode: event.target.value as RewardSelectionMode,
            }))
          }
        >
          <option value="observe">{dictionary.rewards.selectionObserve}</option>
          <option value="enforce">{dictionary.rewards.selectionEnforce}</option>
        </select>
      </label>
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.infoRiskAvoidLevel}
          <Hint content={h.infoRiskAvoidLevel} />
        </span>
        <select
          className={selectClassName}
          value={draft.info_risk_avoid_level}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              info_risk_avoid_level: event.target.value as RewardInfoRiskLevel,
            }))
          }
        >
          <option value="medium">{dictionary.rewards.infoRiskMedium}</option>
          <option value="high">{dictionary.rewards.infoRiskHigh}</option>
          <option value="critical">{dictionary.rewards.infoRiskCritical}</option>
        </select>
      </label>
      <NumberInput
        label={dictionary.rewards.infoRiskTtlSec}
        value={draft.info_risk_ttl_sec}
        suffix="s"
        hint={h.infoRiskTtlSec}
        onChange={(value) => updateNumber("info_risk_ttl_sec", value)}
      />
    </ConfigSection>
  );
}
