Thread.list.find_all { |t|
  t.alive?
}.map { |thread| thread.object_id }
^^^^^ Avoid multi-line chains of blocks.
