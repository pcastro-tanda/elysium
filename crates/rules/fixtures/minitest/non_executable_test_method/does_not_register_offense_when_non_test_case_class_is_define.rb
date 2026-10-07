class SetTest < ActiveRecord::AbstractMysqlTestCase
  class SetTest < ActiveRecord::Base
  end

  def test_should_not_be_unsigned
    column = SetTest.columns_hash["set_column"]
    assert_not_predicate column, :unsigned?
  end
end
