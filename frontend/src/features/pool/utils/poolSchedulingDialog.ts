const DISTRIBUTION_MUTEX_GROUP = 'distribution_mode'

export interface SchedulingDialogPresetLike {
  mutexGroup: string | null
}

function isStrategyPreset(item: SchedulingDialogPresetLike): boolean {
  return item.mutexGroup !== DISTRIBUTION_MUTEX_GROUP
}

export function moveStrategyItem<T extends SchedulingDialogPresetLike>(
  items: readonly T[],
  itemIndex: number,
  direction: -1 | 1,
): T[] {
  const strategyIndexes: number[] = []

  items.forEach((item, index) => {
    if (isStrategyPreset(item)) {
      strategyIndexes.push(index)
    }
  })
  const currentPosition = strategyIndexes.indexOf(itemIndex)
  if (currentPosition === -1) {
    return [...items]
  }

  const targetPosition = currentPosition + direction
  if (targetPosition < 0 || targetPosition >= strategyIndexes.length) {
    return [...items]
  }

  const sourceIndex = strategyIndexes[currentPosition]
  const targetIndex = strategyIndexes[targetPosition]
  const nextItems = [...items]

  ;[nextItems[sourceIndex], nextItems[targetIndex]] = [nextItems[targetIndex], nextItems[sourceIndex]]

  return nextItems
}
