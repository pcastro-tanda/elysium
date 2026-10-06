def mutex
  LOADER.synchronize do
    @mutex ||= Mutex.new
  end
end
