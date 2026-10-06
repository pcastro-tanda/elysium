def monitor
  @monitor ||= Monitor.new
  ^^^^^^^^^^^^^^^^^^^^^^^^ Do not lazily initialize synchronization primitives with `||=`.
end
