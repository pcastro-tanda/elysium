class Test
  SEMAPHORE = Mutex.new
  def self.types
    SEMAPHORE
      .synchronize { @all_types ||= type_class.all }
  end
end
