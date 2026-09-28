Thread.list.find_all { |t| t.alive? }.map { |t|
  t.object_id
}
