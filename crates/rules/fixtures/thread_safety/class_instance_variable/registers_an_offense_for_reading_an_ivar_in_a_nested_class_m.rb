class Test
  define_method :generate_new_class do
    Class.new do
      def self.area
        @area ||= some_computation
        ^^^^^ Avoid class instance variables.
      end
    end
  end
end
