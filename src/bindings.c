#include "bindings.h"

NVSDK_NGX_Resource_VK HELPERS_NVSDK_NGX_Create_ImageView_Resource_VK(
    VkImageView imageView,
    VkImage image,
    VkImageSubresourceRange subresourceRange,
    VkFormat format,
    unsigned int width,
    unsigned int height,
    bool readWrite) {

    return NVSDK_NGX_Create_ImageView_Resource_VK(
        imageView,
        image,
        subresourceRange,
        format,
        width,
        height,
        readWrite
    );
}

NVSDK_NGX_Result HELPERS_NGX_DLSS_GET_OPTIMAL_SETTINGS(
    NVSDK_NGX_Parameter *pInParams,
    unsigned int InUserSelectedWidth,
    unsigned int InUserSelectedHeight,
    NVSDK_NGX_PerfQuality_Value InPerfQualityValue,
    unsigned int *pOutRenderOptimalWidth,
    unsigned int *pOutRenderOptimalHeight,
    unsigned int *pOutRenderMaxWidth,
    unsigned int *pOutRenderMaxHeight,
    unsigned int *pOutRenderMinWidth,
    unsigned int *pOutRenderMinHeight,
    float *pOutSharpness) {

    return NGX_DLSS_GET_OPTIMAL_SETTINGS(
        pInParams,
        InUserSelectedWidth,
        InUserSelectedHeight,
        InPerfQualityValue,
        pOutRenderOptimalWidth,
        pOutRenderOptimalHeight,
        pOutRenderMaxWidth,
        pOutRenderMaxHeight,
        pOutRenderMinWidth,
        pOutRenderMinHeight,
        pOutSharpness
    );
}
