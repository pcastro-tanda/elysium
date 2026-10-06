def mutex
  @mutex ||= Thread::Mutex.new
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not lazily initialize synchronization primitives with `||=`.
end
