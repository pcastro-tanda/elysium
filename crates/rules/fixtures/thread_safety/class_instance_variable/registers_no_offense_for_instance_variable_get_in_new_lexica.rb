class Test
  def self.factory_method
    Class.new do
      def area
        instance_variable_get(:@area)
      end
    end
  end
end
