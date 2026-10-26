// Generate golden coordinates directly from the preserved Arduino mapper.
#include "MatrixOrientation.h"
#include <cstdio>

int main() {
    MatrixOrientation matrix;
    std::printf("[\n");
    for (int i = 0; i < 16; ++i) {
        auto c = matrix.getConfig();
        c.startCorner = i % 4;
        c.panelLayout = (i >> 2) & 1;
        c.panelSerpentine = (i >> 3) & 1;
        c.mirrorX = (i >> 1) & 1;
        for (int p = 0; p < 4; ++p) {
            c.panelOrder[p] = (p + i) % 4;
            c.panelRotation[p] = ((p + i) % 4) * 90;
            c.serpentine[p] = ((i + p) & 1) != 0;
        }
        matrix.begin(c);
        std::printf("{\"startCorner\":%u,\"panelLayout\":%u,\"panelSerpentine\":%s,\"mirrorX\":%s,\"panelOrder\":[", c.startCorner, c.panelLayout, c.panelSerpentine ? "true" : "false", c.mirrorX ? "true" : "false");
        for (int p = 0; p < 4; ++p) std::printf("%s%u", p ? "," : "", c.panelOrder[p]);
        std::printf("],\"panelRotation\":[");
        for (int p = 0; p < 4; ++p) std::printf("%s%u", p ? "," : "", c.panelRotation[p]);
        std::printf("],\"serpentine\":[");
        for (int p = 0; p < 4; ++p) std::printf("%s%s", p ? "," : "", c.serpentine[p] ? "true" : "false");
        std::printf("],\"indices\":[");
        for (int y = 0; y < 32; ++y) for (int x = 0; x < 32; ++x)
            std::printf("%s%u", x || y ? "," : "", matrix.getLEDIndex(x,y));
        std::printf("]}%s\n", i == 15 ? "" : ",");
    }
    std::printf("]\n");
}
