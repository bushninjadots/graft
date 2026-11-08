<script lang="ts">
  import { icons } from "../icons";

  type Props = { name: string; size?: number; stroke?: number };
  let { name, size = 15, stroke = 1.6 }: Props = $props();

  const IconComponent = $derived(icons[name] ?? icons.circle);
</script>

{#if IconComponent}
  <IconComponent size={size} strokeWidth={stroke} />
{/if}
