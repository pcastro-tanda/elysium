class Test
  def self.factory_method
    define_method(:some_method) do |params|
      @params = params
    end
  end
end
