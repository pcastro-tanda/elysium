module Nested
  class MyJob < ActiveJob::Base; end
                ^^^^^^^^^^^^^^^ Jobs should subclass `ApplicationJob`.
end
