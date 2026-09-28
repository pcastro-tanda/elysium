module DPN
  module Flow
    module BaseFlow
      class Start
      end
      def self.included(base)
        def base.Start(aws_env, *args)
        end
      end
    end
  end
end
