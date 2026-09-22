class Cat
  def meow
    puts('Meow!')
  end

  protected

    def can_we_be_friends?(another_cat)
      # some logic
    end

    def related_to?(another_cat)
      # some logic
    end

  private

              # Here we go back an indentation level again. This is a
              # violation of the indented_internal_methods style,
              # but it's not for this cop to report.
              # Layout/IndentationWidth will handle it.
  def meow_at_3am?
    rand < 0.8
  end

              # As long as the indentation of this method is
              # consistent with that of the last one, we're fine.
  def meow_at_4am?
    rand < 0.8
  end
end
