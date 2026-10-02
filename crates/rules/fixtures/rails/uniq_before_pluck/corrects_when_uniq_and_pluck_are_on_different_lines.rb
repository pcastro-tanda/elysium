Model
  .pluck(:name)
  .uniq
   ^^^^ Use `distinct` before `pluck`.
