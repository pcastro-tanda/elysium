validates :colour, format: { message: '%{colour} is not a valid hex colour' }
                                       ^^^^^^^^^ Prefer annotated tokens (like `%<foo>s`) over template tokens (like `%{foo}`).
