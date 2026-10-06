class Test
  def self.factory_method
    Struct.new(:width, :height) do
      def area
        @area ||= width * height
      end
    end
  end
end
