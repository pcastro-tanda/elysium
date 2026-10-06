class Test
  def self.some_method
    @var ||= %w(a b c)
  end
end