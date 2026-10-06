module Test
  @var ||= Concurrent::Map.new do |h, key|
    h.fetch_or_store(key, Concurrent::Map.new)
  end
end