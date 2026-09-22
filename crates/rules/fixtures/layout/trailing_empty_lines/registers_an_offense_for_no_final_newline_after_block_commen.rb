puts 'testing rubocop when final new line is missing
                          after block comments'

=begin
first line
second line
third line
=end
    ^{} Final newline missing.
