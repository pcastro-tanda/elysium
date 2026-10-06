class Test
  def self.factory_method
    Class.new do
      def area
        @area ||= some_computation
      end
    end
  end
end
