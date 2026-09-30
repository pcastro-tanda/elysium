users
  .dup.sort_by { _1.name }
  .select { |u| u.active? }
  .map(&:id)
