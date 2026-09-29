class C
  private

  def self.method
    puts "hi"
  end

  private_class_method :method
end
