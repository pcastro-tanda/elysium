module ActiveRecord
  class AdapterTest < ActiveRecord::TestCase
    unless current_adapter?(:PostgreSQLAdapter)
      def test_update_prepared_statement
      end
    end
  end
end
