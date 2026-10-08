class Test
  def self.factory_method
    Data.define(:width, :height) do
      def area
        @area ||= width * height
      end
    end
  end
end
