class Test
  SEMAPHORE = Mutex.new
  def self.some_method(params)
    SEMAPHORE.synchronize do
      @params = params
    end
  end
end
