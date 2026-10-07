it 'does something' do
  @@n = do_something
  _(@@n[:foo]).must_equal 42
end
