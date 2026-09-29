# comment one
def foobar
  if @foo
  ^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
    # comment 2
    @foo.bar
  end
end
