class Test
  class << self
    def some_method

      @var ||= %i{a b c}
    end
  end
end
