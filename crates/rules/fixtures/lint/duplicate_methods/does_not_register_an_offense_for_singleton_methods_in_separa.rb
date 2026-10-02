def setup
  @first = Class.new do
    class << self
      def name
        'FIRST'
      end
    end
  end

  Class.new do
    class << self
      def name
        'SECOND'
      end
    end
  end
end
