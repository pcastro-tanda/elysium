        puts <<-RUBY2
def foo
^^^^^^^ Use 2 spaces for indentation in a heredoc by using `<<~` instead of `<<-`.
  bar
end
RUBY2
