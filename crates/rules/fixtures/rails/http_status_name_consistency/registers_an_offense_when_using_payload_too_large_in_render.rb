render json: { error: 'File too big' }, status: :payload_too_large
                                                ^^^^^^^^^^^^^^^^^^ Prefer `:content_too_large` over `:payload_too_large`.
