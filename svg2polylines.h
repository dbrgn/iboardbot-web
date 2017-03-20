typedef struct CoordinatePair {
    double x;
    double y;
} CoordinatePair;

typedef struct Polyline {
    CoordinatePair* ptr;
    size_t len;
} Polyline;

uint8_t svg_str_to_polylines(char* svg, Polyline** polylines, size_t* polylines_len);
